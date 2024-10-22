use async_channel;
use async_channel::{Receiver, Sender, TryRecvError};
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::stream;
use crate::stream::Frame;

pub use async_channel::SendError;

#[derive(Debug)]
pub enum OutputError {
    DeviceClosed,
}

pub trait Output {
    fn push(&mut self, frame: Frame) -> Result<(), OutputError>;
}

pub struct OutputDevice {
    sender: Sender<Frame>,
    _stream: Box<dyn StreamTrait>,
}

#[derive(Debug)]
pub enum OpenError {
    DeviceNotAvailable,
    ConfigNotAvailable,
    BuildStreamError(cpal::BuildStreamError),
    PlayStreamError,
}

impl OutputDevice {
    /// The buffer size to request that the device uses when it calls for more
    /// samples.
    /// Empirically, ALSA doesn't respect this very well, although it does seem
    /// to have _some_ influence
    /// I observed some crackling when this was set to 256 and 512...
    /// 512 samples at 44.1kHz ~= 24ms
    pub const DEVICE_BUFFER: cpal::FrameCount = 1024;

    /// The max number of `Frame`s waiting to be output.
    /// In the common cases (playback from file, synthesis) samples can be
    /// produced much faster than they are output, this limit is what applies
    /// backpressure, and so long as this is sufficiently high that data is
    /// always available when the hardware wants it, setting this higher just
    /// increases memory use and output latency.
    const MAX_FRAME_QUEUE_LEN: usize = 4;

    pub fn new(
        channels: stream::ChannelCount,
        sample_rate: stream::SampleRate,
    ) -> Result<OutputDevice, OpenError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(OpenError::DeviceNotAvailable)?;

        let mut supported: Option<cpal::SupportedStreamConfigRange> = None;
        for c in device
            .supported_output_configs()
            .or(Err(OpenError::ConfigNotAvailable))?
        {
            if let SampleFormat::F32 = c.sample_format() {
                if c.channels() == u16::from(channels) {
                    supported = Some(c);
                    break;
                }
            }
        }

        // TODO: what if the desired configuration isn't supported?
        let mut config = supported
            .ok_or(OpenError::ConfigNotAvailable)?
            .with_sample_rate(cpal::SampleRate::from(sample_rate))
            .config();
        config.buffer_size = cpal::BufferSize::Fixed(OutputDevice::DEVICE_BUFFER);

        let (sender, receiver) = async_channel::bounded(OutputDevice::MAX_FRAME_QUEUE_LEN);
        let mut receiver = FrameReceiver::new(channels, sample_rate, receiver);
        let stream = Box::new(
            device
                .build_output_stream(
                    &config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| match receiver
                        .fill_buffer(data)
                    {
                        Ok(satisfied) => {
                            if satisfied == 0 {
                                println!("Dropped output!");
                            } else if satisfied < data.len() {
                                println!("Underfull output: {} < {}!", satisfied, data.len());
                            }
                        }
                        Err(FrameReceiverError::EndOfStream) => {
                            println!("At end of stream.")
                        }
                    },
                    move |_err| {
                        todo!();
                    },
                    None, // blocking (??)
                )
                .map_err(|e| OpenError::BuildStreamError(e))?,
        );
        stream.play().or(Err(OpenError::PlayStreamError))?;

        Ok(OutputDevice {
            sender,
            _stream: stream,
        })
    }
}

impl Output for OutputDevice {
    fn push(&mut self, frame: Frame) -> Result<(), OutputError> {
        self.sender
            .send_blocking(frame)
            .map_err(|_| OutputError::DeviceClosed)
    }
}

/// Wraps an async_channel::Receiver<Frame> with logic to copy sample data
/// from Frames into the buffers that the output device has requested be filled.
struct FrameReceiver {
    channels: stream::ChannelCount,
    sample_rate: stream::SampleRate,
    receiver: Receiver<Frame>,
    cur_frame: Option<Frame>,
    cur_sample: Option<usize>, // Some iff samples remain in cur_frame
}

#[derive(Clone, Copy, Debug)]
enum FrameReceiverError {
    EndOfStream,
}

impl FrameReceiver {
    fn new(
        channels: stream::ChannelCount,
        sample_rate: stream::SampleRate,
        receiver: Receiver<Frame>,
    ) -> FrameReceiver {
        FrameReceiver {
            channels,
            sample_rate,
            receiver,
            cur_frame: None,
            cur_sample: None,
        }
    }

    /// Fill the given output buffer with samples.
    /// @return the number of samples returned, which may be less than the
    ///     length of @p buf if insufficient samples are currently queued.
    fn fill_buffer(&mut self, buf: &mut [f32]) -> Result<usize, FrameReceiverError> {
        let mut satisfied: usize = 0;

        while satisfied < buf.len() {
            match self.next_slice(buf.len() - satisfied) {
                Ok(Some(slice)) => {
                    buf[satisfied..satisfied + slice.len()].copy_from_slice(slice);
                    satisfied += slice.len();
                }
                Ok(None) => return Ok(satisfied),
                Err(e) => return Err(e),
            };
        }

        Ok(satisfied)
    }

    /// Return a slice of the next available samples from the next frame in the
    /// stream, up to either @p max_len or the end of the frame.
    /// @return None if the next frame is not currently queued.
    fn next_slice(&mut self, max_len: usize) -> Result<Option<&[f32]>, FrameReceiverError> {
        if let Some(cur_sample) = self.cur_sample {
            // Have some remainder in the current frame
            Ok(Some(self.next_slice_from_current(cur_sample, max_len)))
        } else {
            // Have returned the entire previous frame; try to get the next
            match self.receiver.try_recv() {
                Ok(next) => {
                    assert!(next.channels == self.channels);
                    assert!(next.sample_rate == self.sample_rate);
                    self.cur_frame = Some(next);
                    self.cur_sample = Some(0);
                    Ok(Some(self.next_slice_from_current(0, max_len)))
                }
                Err(TryRecvError::Empty) => Ok(None),
                Err(TryRecvError::Closed) => Err(FrameReceiverError::EndOfStream),
            }
        }
    }

    // Helper: get a slice from self.cur_frame and update self.cur_sample
    fn next_slice_from_current(&mut self, cur_sample: usize, max_len: usize) -> &[f32] {
        // if self.cur_sample is Some, self.cur_frame must be Some:
        let frame = self.cur_frame.as_ref().expect("impossible");

        let start = cur_sample;
        let end = (start + max_len).min(frame.samples.len());
        // cur_sample must be before the end of self.cur_frame:
        assert!(start < end);

        // Update cur_sample depending on if we're at end-of-frame yet
        if end < frame.samples.len() {
            self.cur_sample = Some(end);
        } else {
            self.cur_sample = None;
        }

        &frame.samples[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recv_next_slice() {
        let channels = stream::ChannelCount(1);
        let sample_rate = stream::SampleRate(2);
        let (send, recv) = async_channel::unbounded::<Frame>();
        let mut iter = FrameReceiver::new(channels, sample_rate, recv);
        assert!(iter.next_slice(42).unwrap().is_none());

        // Send a frame...
        let f1 = Frame {
            channels,
            sample_rate,
            samples: vec![1., 2., 3., 4.],
        };
        send.send_blocking(f1).unwrap();
        // Should be able to get just the first 3/4 samples:
        assert_eq!(iter.next_slice(3).unwrap().unwrap(), [1., 2., 3.]);
        // And then just the remaining sample:
        assert_eq!(iter.next_slice(42).unwrap().unwrap(), [4.]);
        // And then nothing!
        assert!(iter.next_slice(42).unwrap().is_none());

        // Send another frame...
        let f2 = Frame {
            channels,
            sample_rate,
            samples: vec![5., 6., 7., 8.],
        };
        send.send_blocking(f2).unwrap();
        // Should be able to get the entire frame:
        assert_eq!(iter.next_slice(42).unwrap().unwrap(), [5., 6., 7., 8.]);

        // And, if another frame is received before we call next_slice again,
        // it should be returned
        let f3 = Frame {
            channels,
            sample_rate,
            samples: vec![9., 10.],
        };
        send.send_blocking(f3).unwrap();
        assert_eq!(iter.next_slice(42).unwrap().unwrap(), [9., 10.])
    }

    #[test]
    fn test_fill_buf() {
        let channels = stream::ChannelCount(1);
        let sample_rate = stream::SampleRate(2);
        let (send, recv) = async_channel::unbounded::<Frame>();
        let mut iter = FrameReceiver::new(channels, sample_rate, recv);

        // Send a few frames...
        send.send_blocking(Frame {
            channels,
            sample_rate,
            samples: vec![1., 2.],
        })
        .unwrap();
        send.send_blocking(Frame {
            channels,
            sample_rate,
            samples: vec![3., 4.],
        })
        .unwrap();
        send.send_blocking(Frame {
            channels,
            sample_rate,
            samples: vec![5., 6.],
        })
        .unwrap();

        // Try to receive most of them (spanning all 3 frames)
        let mut buf = [0f32; 5];
        assert_eq!(iter.fill_buffer(&mut buf[..]).unwrap(), 5);
        assert_eq!(buf, [1., 2., 3., 4., 5.]);

        // And then get the last bit, which underfills the buffer:
        let mut buf = [0f32; 2];
        assert_eq!(iter.fill_buffer(&mut buf[..]).unwrap(), 1);
        assert_eq!(buf, [6., 0.]);
    }
}
