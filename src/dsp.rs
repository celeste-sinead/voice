use crate::stream::buffer::ChannelPeriod;

pub fn rms(period: &ChannelPeriod) -> f32 {
    let sum_sq = period.iter().fold(0.0, |acc, x| acc + x * x);
    let mean_sq = sum_sq / period.len() as f32;
    mean_sq.sqrt()
}
