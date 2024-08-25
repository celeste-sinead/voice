use std::cmp;
use std::collections::VecDeque;

use super::input::Frame;

#[allow(dead_code)] // TODO: until it's used
pub struct InputBuffer<const CHANNELS: usize, const SAMPLE_RATE: u32> {
    max_len: usize,
    buffers: [VecDeque<f32>; CHANNELS],
}

#[allow(dead_code)] // TODO: until it's used
impl<const CHANNELS: usize, const SAMPLE_RATE: u32> InputBuffer<CHANNELS, SAMPLE_RATE> {
    pub fn new(max_len: usize) -> InputBuffer<CHANNELS, SAMPLE_RATE> {
        let buffers: [VecDeque<f32>; CHANNELS] = [(); CHANNELS].map(|_| {
            let mut v = VecDeque::new();
            v.reserve_exact(max_len);
            v
        });
        InputBuffer { max_len, buffers }
    }

    pub fn push(&mut self, f: &Frame) {
        assert!(f.channels as usize == CHANNELS);
        assert!(f.sample_rate == SAMPLE_RATE);

        // De-interlace samples into buffers:
        assert!(f.samples.len() % CHANNELS == 0);
        for (i, s) in f.samples.iter().enumerate() {
            let ch = i % CHANNELS;
            if self.buffers[ch].len() == self.max_len {
                self.buffers[ch].pop_front();
            }
            self.buffers[ch].push_back(*s);
        }
    }

    /// Peek at the last n samples in the more recent segment of the ring
    /// buffer, returning fewer if n are not available.
    fn get_last(&self, channel: usize, n: usize) -> &[f32] {
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
        let mut buf: InputBuffer<2, 44100> = InputBuffer::new(100);
        buf.push(&Frame {
            number: 0,
            channels: 2,
            sample_rate: 44100,
            samples: vec![1., 2., 3., 4.],
        });
        assert_eq!(buf.get_last(0, 2), [1., 3.]);
        assert_eq!(buf.get_last(1, 2), [2., 4.]);
    }

    #[test]
    fn wrap_around() {
        let mut buf: InputBuffer<1, 44100> = InputBuffer::new(4);
        // Add 3 1's, almost filling the max length of 4
        buf.push(&Frame {
            number: 0,
            channels: 1,
            sample_rate: 44100,
            samples: vec![1.; 3],
        });
        // Add 2 2's, filling the ring, and then replacing the first 1
        buf.push(&Frame {
            number: 0,
            channels: 1,
            sample_rate: 44100,
            samples: vec![2.; 2],
        });
        // The ring should have wrapped around and therefore be split
        // into two slices:
        assert_eq!(
            buf.buffers[0].as_slices(),
            ([1., 1., 2.].as_slice(), [2.].as_slice())
        );
    }
}
