use std::cmp;
use std::collections::VecDeque;
use std::iter;
use std::slice;

use super::input::{ChannelCount, Frame, SampleRate};

/// A set of per-channel ringbuffers. This accomplishes two things:
/// - de-interlaces the samples we receive from the device, because ~everything
///   we want to do will want to operate on contiguous data for each channel
/// - allow us to adapt from whatever buffer size the device is using to
///   whatever period we want to use for processing (e.g. for FFTs)
#[allow(dead_code)] // TODO: until it's used
pub struct InputBuffer {
    max_len: usize,
    channels: ChannelCount,
    sample_rate: SampleRate,
    buffers: Vec<VecDeque<f32>>,
    sample_count: usize,
}

#[allow(dead_code)] // TODO: until it's used
impl InputBuffer {
    pub fn new(channels: ChannelCount, sample_rate: SampleRate, max_len: usize) -> InputBuffer {
        let mut buffers = Vec::new();
        for _ in 0..usize::from(channels) {
            let mut b = VecDeque::new();
            b.reserve_exact(max_len);
            buffers.push(b);
        }
        InputBuffer {
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

    pub fn len(&self) -> usize {
        return cmp::min(self.sample_count, self.max_len);
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

/// A reference to a contiguous sequence of samples in an InputBuffer
#[allow(dead_code)] // TODO: until it's used
pub struct Period<'a> {
    buffer: &'a InputBuffer,
    start_sample_num: usize,
    len: usize,
}

#[allow(dead_code)] // TODO: until it's used
impl<'a> Period<'a> {
    /// Return the samples in the given channel, as a pair of consecutive slices
    pub fn get_channel_slices(&'a self, channel: usize) -> (&'a [f32], &'a [f32]) {
        // Get all available samples, as 1-2 slices of ring buffer
        let (first_segment, second_segment) = self.buffer.buffers[channel].as_slices();

        // Figure out where this period starts and ends, relative to the indices
        // of the first ring segment:
        let len_to_buffer_end = self.buffer.sample_count - self.start_sample_num;
        // (checked to catch the case where this period is no longer in the ring)
        let mut start = self.buffer.len().checked_sub(len_to_buffer_end).unwrap();
        let mut end = start + self.len;

        // Figure out if the period is in the first ring segment
        let first = if start < first_segment.len() {
            if end <= first_segment.len() {
                // It's entirely in the first segment
                return (&first_segment[start..end], &[]);
            } else {
                // It's split between the first and second segments
                let slice = &first_segment[start..];
                start = 0; // relative to the second segment
                end -= first_segment.len();
                slice
            }
        } else {
            // It's entirely in the second segment
            start -= first_segment.len(); // relative to the second segment
            end -= first_segment.len();
            &[]
        };

        if first.len() > 0 {
            (first, &second_segment[start..end])
        } else {
            (&second_segment[start..end], &[])
        }
    }

    /// Iterate over the samples in one channel
    pub fn iter_channel(
        &'a self,
        channel: usize,
    ) -> iter::Chain<slice::Iter<'a, f32>, slice::Iter<'a, f32>> {
        let (a, b) = self.get_channel_slices(channel);
        a.iter().chain(b.iter())
    }
}

/// Produces a stream of periods, as they become available in an InputBuffer
#[allow(dead_code)] // TODO: until it's used
pub struct PeriodStream {
    buffer: InputBuffer,
    period_len: usize,
    period_stride: usize,
    next_period_end: usize,
}

#[allow(dead_code)] // TODO: until it's used
impl PeriodStream {
    /// A stream of Periods of length period_len, with the start/end advancing
    /// by period_stride for each subsequent period. (if the stride is less than
    /// the length, periods will overlap).
    pub fn new(buffer: InputBuffer, period_len: usize, period_stride: usize) -> PeriodStream {
        // the buffer must initially contain the first sample:
        assert!(buffer.sample_count <= buffer.max_len);
        PeriodStream {
            buffer,
            period_len,
            period_stride,
            next_period_end: period_len,
        }
    }

    pub fn push(&mut self, f: &Frame) {
        self.buffer.push(f)
    }

    /// Get the next available Period, if any
    /// Note that this is intentionally not implementing Iterator, because the
    /// PeriodStream should not be consumed, because it may return additional
    /// Periods once more frames are pushed.
    pub fn next(&mut self) -> Option<Period> {
        if self.next_period_end <= self.buffer.sample_count {
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

    #[test]
    fn basic_period_stream() {
        let mut stream = PeriodStream::new(
            InputBuffer::new(ChannelCount::new(1), SampleRate::new(44100), 100),
            4,
            2,
        );
        stream.push(&Frame {
            number: 0,
            channels: ChannelCount::new(1),
            sample_rate: SampleRate::new(44100),
            samples: (1..8).map(|x| x as f32).collect(),
        });

        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel_slices(0);
            assert_eq!(a, [1., 2., 3., 4.]);
            assert_eq!(b, []);
        } else {
            panic!("expected period");
        }

        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel_slices(0);
            assert_eq!(a, [3., 4., 5., 6.]);
            assert_eq!(b, []);
        } else {
            panic!("expected period");
        }

        assert!(stream.next().is_none());

        stream.push(&Frame {
            number: 0,
            channels: ChannelCount::new(1),
            sample_rate: SampleRate::new(44100),
            samples: (8..9).map(|x| x as f32).collect(),
        });

        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel_slices(0);
            assert_eq!(a, [5., 6., 7., 8.]);
            assert_eq!(b, []);
        } else {
            panic!("expected period");
        }
    }

    #[test]
    fn periods_split_ring() {
        // Fill an 8-sample ring buffer (but don't wrap yet)
        let mut stream = PeriodStream::new(
            InputBuffer::new(ChannelCount::new(1), SampleRate::new(44100), 8),
            4,
            2,
        );
        stream.push(&Frame {
            number: 0,
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
            let (a, b) = p.get_channel_slices(0);
            assert_eq!(a, [4., 5., 6., 7.]);
            assert_eq!(b, []);
        } else {
            panic!("expected period");
        }

        // Add some more samples, which should produce a split ring:
        stream.push(&Frame {
            number: 0,
            channels: ChannelCount::new(1),
            sample_rate: SampleRate::new(44100),
            samples: (8..12).map(|x| x as f32).collect(),
        });

        // And the next period should be split between sample 7 and 8:
        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel_slices(0);
            assert_eq!(a, [6., 7.]);
            assert_eq!(b, [8., 9.]);
            let v: Vec<f32> = p.iter_channel(0).map(|x| *x).collect();
            assert_eq!(v, [6., 7., 8., 9.])
        } else {
            panic!("expected period");
        }

        // And then the next sample won't be split, but is interesting
        // because, internally it's entirely within the second ring segment
        if let Some(p) = stream.next() {
            let (a, b) = p.get_channel_slices(0);
            assert_eq!(a, [8., 9., 10., 11.]);
            assert_eq!(b, []);
        } else {
            panic!("expected period");
        }
    }
}
