use std::cmp;
use std::collections::VecDeque;

use super::input::{ChannelCount, Frame, SampleRate};

#[allow(dead_code)] // TODO: until it's used
pub struct InputBuffer {
    max_len: usize,
    channels: ChannelCount,
    sample_rate: SampleRate,
    buffers: Vec<VecDeque<f32>>,
}

#[allow(dead_code)] // TODO: until it's used
impl InputBuffer {
    pub fn new(channels: ChannelCount, sample_rate: SampleRate, max_len: usize) -> InputBuffer {
        let mut buffers = Vec::new();
        for _ in 0..channels.into() {
            let mut b = VecDeque::new();
            b.reserve_exact(max_len);
            buffers.push(b);
        }
        InputBuffer {
            max_len,
            channels,
            sample_rate,
            buffers,
        }
    }

    pub fn push(&mut self, f: &Frame) {
        assert!(f.channels == self.channels);
        assert!(f.sample_rate == self.sample_rate);

        // De-interlace samples into buffers:
        assert!(f.samples.len() % u16::from(self.channels) as usize == 0);
        for (i, s) in f.samples.iter().enumerate() {
            let ch = i % u16::from(self.channels) as usize;
            if self.buffers[ch].len() == self.max_len {
                self.buffers[ch].pop_front();
            }
            self.buffers[ch].push_back(*s);
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deinterlacing() {
        let mut buf: InputBuffer =
            InputBuffer::new(ChannelCount::new(2), SampleRate::new(44100), 100);
        buf.push(&Frame {
            number: 0,
            channels: ChannelCount::new(2),
            sample_rate: SampleRate::new(44100),
            samples: vec![1., 2., 3., 4.],
        });
        assert_eq!(buf.peek_tail(0, 2), [1., 3.]);
        assert_eq!(buf.peek_tail(1, 2), [2., 4.]);
    }

    #[test]
    fn wrap_around() {
        let mut buf: InputBuffer =
            InputBuffer::new(ChannelCount::new(1), SampleRate::new(44100), 4);
        // Add 3 1's, almost filling the max length of 4
        buf.push(&Frame {
            number: 0,
            channels: ChannelCount::new(1),
            sample_rate: SampleRate::new(44100),
            samples: vec![1.; 3],
        });
        // Add 2 2's, filling the ring, and then replacing the first 1
        buf.push(&Frame {
            number: 0,
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
}
