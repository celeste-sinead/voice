use std::cmp;
use std::collections::VecDeque;
use std::iter;
use std::marker::PhantomData;
use std::mem;
use std::slice;

use super::input::{ChannelCount, Frame, Input, InputAdapter, InputError, Instant, SampleRate};
use super::pipeline::Step;

/// A set of per-channel ringbuffers. This accomplishes two things:
/// - de-interlaces the samples we receive from the device, because ~everything
///   we want to do will want to operate on contiguous data for each channel
/// - allow us to adapt from whatever buffer size the device is using to
///   whatever period we want to use for processing (e.g. for FFTs)
pub struct SampleBuffer {
    max_len: usize,
    channels: ChannelCount,
    sample_rate: SampleRate,
    buffers: Vec<VecDeque<f32>>,
    sample_count: usize,
}

impl SampleBuffer {
    pub fn new(channels: ChannelCount, sample_rate: SampleRate, max_len: usize) -> SampleBuffer {
        let mut buffers = Vec::new();
        for _ in 0..usize::from(channels) {
            let mut b = VecDeque::new();
            b.reserve_exact(max_len);
            buffers.push(b);
        }
        SampleBuffer {
            max_len,
            channels,
            sample_rate,
            buffers,
            sample_count: 0,
        }
    }

    pub fn push(&mut self, f: &Frame) {
        assert!(f.channels == self.channels);
        assert!(f.sample_rate == self.sample_rate);

        // De-interlace samples into buffers:
        assert!(f.samples.len() % usize::from(self.channels) == 0);
        self.sample_count += f.samples.len() / usize::from(self.channels);
        for (i, s) in f.samples.iter().enumerate() {
            let ch = i % usize::from(self.channels);
            if self.buffers[ch].len() == self.max_len {
                self.buffers[ch].pop_front();
            }
            self.buffers[ch].push_back(*s);
        }
    }

    fn len(&self) -> usize {
        return cmp::min(self.sample_count, self.max_len);
    }

    fn oldest_sample_index(&self) -> usize {
        self.sample_count - self.len()
    }
}

#[cfg(test)]
impl SampleBuffer {
    /// Peek at the last n samples in the more recent segment of the ring
    /// buffer, returning fewer if n are not available.
    fn peek_tail(&self, channel: usize, n: usize) -> &[f32] {
        let (a, b) = self.buffers[channel].as_slices();
        if b.len() == 0 {
            let avail = cmp::min(a.len(), n);
            &a[a.len() - avail..]
        } else {
            let avail = cmp::min(b.len(), n);
            &b[b.len() - avail..]
        }
    }
}

/// A reference to a contiguous sequence of samples in an SampleBuffer
pub struct Period<'a> {
    buffer: &'a SampleBuffer,
    start_sample_num: usize,
    len: usize,
}

impl<'a> Period<'a> {
    pub fn channel_count(&self) -> ChannelCount {
        self.buffer.channels
    }

    pub fn get_channel(&'a self, channel: usize) -> ChannelPeriod<'a> {
        // Get all available samples, as 1-2 slices of ring buffer
        let (first_segment, second_segment) = self.buffer.buffers[channel].as_slices();

        // Figure out where this period starts and ends, relative to the indices
        // of the first ring segment:
        let len_to_buffer_end = self.buffer.sample_count - self.start_sample_num;
        // (checked to catch the case where this period is no longer in the ring)
        let mut start = self.buffer.len().checked_sub(len_to_buffer_end).unwrap();
        let mut end = start + self.len;

        // Figure out where this period is in the ring segments:
        let slices: (&[f32], &[f32]) = if start < first_segment.len() {
            // At least part of the period is in the first segment...
            if end <= first_segment.len() {
                // It's entirely in the first segment
                (&first_segment[start..end], &[])
            } else {
                // It's split between the first and second segments
                let first = &first_segment[start..];
                start = 0;
                end -= first_segment.len();
                (first, &second_segment[start..end])
            }
        } else {
            // It's entirely in the second segment
            start -= first_segment.len();
            end -= first_segment.len();
            (&second_segment[start..end], &[])
        };

        ChannelPeriod {
            slices,
            sample_rate: self.buffer.sample_rate,
            start_sample_num: self.start_sample_num,
            len: self.len,
        }
    }

    pub fn channels(&'a self) -> Vec<ChannelPeriod<'a>> {
        (0..usize::from(self.buffer.channels))
            .map(|i| self.get_channel(i))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn sample_rate(&self) -> SampleRate {
        self.buffer.sample_rate
    }

    pub fn start_time(&self) -> Instant {
        Instant::from_sample_num(self.start_sample_num, self.buffer.sample_rate)
    }

    pub fn end_time(&self) -> Instant {
        Instant::from_sample_num(self.start_sample_num + self.len, self.buffer.sample_rate)
    }
}

/// A contiguous period of samples in a single channel
pub struct ChannelPeriod<'a> {
    pub slices: (&'a [f32], &'a [f32]),
    sample_rate: SampleRate,
    start_sample_num: usize,
    len: usize,
}

impl<'a> ChannelPeriod<'a> {
    pub fn iter(&'a self) -> iter::Chain<slice::Iter<'a, f32>, slice::Iter<'a, f32>> {
        self.slices.0.iter().chain(self.slices.1.iter())
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    pub fn into_timeseries(self) -> TimeseriesIterator<'a> {
        TimeseriesIterator {
            period: self,
            first_slice: true,
            index: 0,
        }
    }
}

pub struct TimeseriesIterator<'a> {
    period: ChannelPeriod<'a>,
    first_slice: bool,
    index: usize,
}

impl<'a> Iterator for TimeseriesIterator<'a> {
    type Item = (Instant, f32);
    fn next(&mut self) -> Option<Self::Item> {
        let slice = if self.first_slice {
            self.period.slices.0
        } else {
            self.period.slices.1
        };
        if self.index < slice.len() {
            let res = Some((
                Instant::from_sample_num(
                    self.period.start_sample_num + self.index,
                    self.period.sample_rate,
                ),
                slice[self.index],
            ));
            self.index += 1;
            res
        } else {
            if self.first_slice {
                self.first_slice = false;
                self.next()
            } else {
                None
            }
        }
    }
}

/// Produces a stream of periods, as they become available in an SampleBuffer
pub struct PeriodBuffer {
    buffer: SampleBuffer,
    period_len: usize,
    period_stride: usize,
    next_period_end: usize,
}

impl PeriodBuffer {
    /// A stream of Periods of length period_len, with the start/end advancing
    /// by period_stride for each subsequent period. (if the stride is less than
    /// the length, periods will overlap).
    pub fn new(buffer: SampleBuffer, period_len: usize, period_stride: usize) -> PeriodBuffer {
        // the buffer must initially contain the first sample:
        assert!(buffer.sample_count <= buffer.max_len);
        PeriodBuffer {
            buffer,
            period_len,
            period_stride,
            next_period_end: period_len,
        }
    }

    pub fn push(&mut self, f: &Frame) {
        self.buffer.push(f);
        // Verify the start of the buffer hasn't moved past the start of the
        // next period, which might happen if too many samples get pushed
        // between calls to next()
        let next_period_start = self.next_period_end - self.period_len;
        assert!(
            next_period_start >= self.buffer.oldest_sample_index(),
            "next_period_start = {}, oldest_sample_index = {}",
            next_period_start,
            self.buffer.oldest_sample_index()
        );
    }

    pub fn has_next(&self) -> bool {
        self.next_period_end <= self.buffer.sample_count
    }

    /// Get the next available Period, if any
    pub fn next(&mut self) -> Option<Period> {
        if self.has_next() {
            let period = Period {
                buffer: &self.buffer,
                len: self.period_len,
                start_sample_num: self.next_period_end - self.period_len,
            };
            self.next_period_end += self.period_stride;
            Some(period)
        } else {
            None
        }
    }
}

pub struct BufferedInput<'a, T: Input<'a, Item = Frame>> {
    input: T,
    buffer: PeriodBuffer,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, T> BufferedInput<'a, T>
where
    T: Input<'a, Item = Frame> + 'a,
{
    /// The BufferedInput will get its sample rate and channel count from the input
    pub fn new(input: T, period_len: usize) -> Result<BufferedInput<'a, T>, InputError> {
        // let frame = input.read()?;
        let buffer = PeriodBuffer::new(
            SampleBuffer::new(ChannelCount::new(1), SampleRate::new(1), 2 * period_len),
            period_len,
            period_len,
        );
        // buffer.push(&frame);
        Ok(BufferedInput {
            input,
            buffer,
            _phantom: PhantomData,
        })
    }

    pub fn next(&mut self) -> Result<Period, InputError> {
        // Read from the input until a full period is available
        todo!()
        // while !self.buffer.has_next() {
        //     let frame = self.input.read()?;
        //     self.buffer.push(&frame);
        // }
        // Ok(self.buffer.next().unwrap())
    }
}

impl<'a, T> BufferedInput<'a, InputAdapter<'a, T, FrameAccumulator>>
where
    T: Input<'a, Item = f32> + 'a,
{
    pub fn from_sample_input(
        input: T,
        channels: ChannelCount,
        sample_rate: SampleRate,
        period_len: usize,
    ) -> Result<BufferedInput<'a, InputAdapter<'a, T, FrameAccumulator>>, InputError> {
        BufferedInput::new(
            InputAdapter::new(
                input,
                FrameAccumulator::new(channels, sample_rate, period_len),
            ),
            period_len,
        )
    }
}

/// Accumulates interlaced samples into `Frame`s.
pub struct FrameAccumulator {
    channels: ChannelCount,
    sample_rate: SampleRate,
    frame_len: usize,
    samples: Vec<f32>,
}

impl FrameAccumulator {
    // Smallish for tests that want to use small buffers; it probably doesn't
    // really matter what this is set to most of the time
    pub const DEFAULT_FRAME_LEN: usize = 16;

    pub fn new(
        channels: ChannelCount,
        sample_rate: SampleRate,
        frame_len: usize,
    ) -> FrameAccumulator {
        assert_eq!(frame_len % usize::from(channels), 0);
        FrameAccumulator {
            channels,
            sample_rate,
            frame_len,
            samples: Vec::with_capacity(frame_len),
        }
    }

    pub fn with_frame_len(mut self, new_len: usize) -> Self {
        assert!(self.samples.is_empty());
        self.frame_len = new_len;
        self.samples.reserve_exact(new_len);
        self
    }
}

impl Step<'_> for FrameAccumulator {
    type Input = f32;
    type Output = Frame;
    type Result = Option<Frame>;

    fn process(&mut self, input: f32) -> Option<Frame> {
        self.samples.push(input);
        if self.samples.len() == self.frame_len {
            let mut res = Frame {
                channels: self.channels,
                sample_rate: self.sample_rate,
                samples: Vec::with_capacity(self.frame_len),
            };
            mem::swap(&mut res.samples, &mut self.samples);
            Some(res)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deinterlacing() {
        let mut buf: SampleBuffer =
            SampleBuffer::new(ChannelCount::new(2), SampleRate::new(44100), 100);
        buf.push(&Frame {
            channels: ChannelCount::new(2),
            sample_rate: SampleRate::new(44100),
            samples: vec![1., 2., 3., 4.],
        });
        assert_eq!(buf.peek_tail(0, 2), [1., 3.]);
        assert_eq!(buf.peek_tail(1, 2), [2., 4.]);
    }

    #[test]
    fn wrap_around() {
        let mut buf: SampleBuffer =
            SampleBuffer::new(ChannelCount::new(1), SampleRate::new(44100), 4);
        // Add 3 1's, almost filling the max length of 4
        buf.push(&Frame {
            channels: ChannelCount::new(1),
            sample_rate: SampleRate::new(44100),
            samples: vec![1.; 3],
        });
        // Add 2 2's, filling the ring, and then replacing the first 1
        buf.push(&Frame {
            channels: ChannelCount::new(1),
            sample_rate: SampleRate::new(44100),
            samples: vec![2.; 2],
        });
        // The ring should have wrapped around and therefore be split
        // into two slices. It is important that this happens because it proves
        // that the ringbuffer didn't get accidentally rotated
        assert_eq!(
            buf.buffers[0].as_slices(),
            ([1., 1., 2.].as_slice(), [2.].as_slice())
        );
    }

    #[test]
    fn basic_period_stream() {
        let mut stream = PeriodBuffer::new(
            SampleBuffer::new(ChannelCount::new(1), SampleRate::new(44100), 100),
            4,
            2,
        );
        stream.push(&Frame {
            channels: ChannelCount::new(1),
            sample_rate: SampleRate::new(44100),
            samples: (1..8).map(|x| x as f32).collect(),
        });

        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel(0).slices;
            assert_eq!(a, [1., 2., 3., 4.]);
            assert_eq!(b, []);
        } else {
            panic!("expected period");
        }

        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel(0).slices;
            assert_eq!(a, [3., 4., 5., 6.]);
            assert_eq!(b, []);
        } else {
            panic!("expected period");
        }

        assert!(stream.next().is_none());

        stream.push(&Frame {
            channels: ChannelCount::new(1),
            sample_rate: SampleRate::new(44100),
            samples: (8..9).map(|x| x as f32).collect(),
        });

        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel(0).slices;
            assert_eq!(a, [5., 6., 7., 8.]);
            assert_eq!(b, []);
        } else {
            panic!("expected period");
        }
    }

    #[test]
    fn periods_split_ring() {
        // Fill an 8-sample ring buffer (but don't wrap yet)
        let mut stream = PeriodBuffer::new(
            SampleBuffer::new(ChannelCount::new(1), SampleRate::new(44100), 8),
            4,
            2,
        );
        stream.push(&Frame {
            channels: ChannelCount::new(1),
            sample_rate: SampleRate::new(44100),
            samples: (0..8).map(|x| x as f32).collect(),
        });

        // First two periods are covered by the basic stream test
        for _ in 0..2 {
            assert!(stream.next().is_some());
        }

        // Should be able to get the period that reaches the end of the stream
        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel(0).slices;
            assert_eq!(a, [4., 5., 6., 7.]);
            assert_eq!(b, []);
        } else {
            panic!("expected period");
        }

        // Add some more samples, which should produce a split ring:
        stream.push(&Frame {
            channels: ChannelCount::new(1),
            sample_rate: SampleRate::new(44100),
            samples: (8..12).map(|x| x as f32).collect(),
        });

        // And the next period should be split between sample 7 and 8:
        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel(0).slices;
            assert_eq!(a, [6., 7.]);
            assert_eq!(b, [8., 9.]);
            let v: Vec<f32> = p.get_channel(0).iter().map(|x| *x).collect();
            assert_eq!(v, [6., 7., 8., 9.])
        } else {
            panic!("expected period");
        }

        // And then the next sample won't be split, but is interesting
        // because, internally it's entirely within the second ring segment
        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel(0).slices;
            assert_eq!(a, [8., 9., 10., 11.]);
            assert_eq!(b, []);
        } else {
            panic!("expected period");
        }
    }

    #[test]
    fn test_frame_accumulator() {
        let mut accum = FrameAccumulator::new(ChannelCount::new(1), SampleRate::new(44100), 4);
        for i in 0..3 {
            assert!(accum.process(i as f32).is_none());
        }
        let f = accum.process(3.).unwrap();
        assert_eq!(f.samples, [0., 1., 2., 3.]);

        for i in 4..7 {
            assert!(accum.process(i as f32).is_none());
        }
        let f = accum.process(7.).unwrap();
        assert_eq!(f.samples, [4., 5., 6., 7.]);
    }
}
