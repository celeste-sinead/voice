use crate::stream::buffer::ChannelPeriod;

pub fn rms(period: &ChannelPeriod) -> f32 {
    let sum_sq = period.iter().fold(0.0, |acc, x| acc + x * x);
    let mean_sq = sum_sq / period.len() as f32;
    mean_sq.sqrt()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decibels(f32);

impl Decibels {
    pub fn from_full_scale(fs: f32) -> Decibels {
        Decibels(10. * fs.log10())
    }
}

impl From<Decibels> for f32 {
    fn from(db: Decibels) -> f32 {
        db.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_from_full_scale() {
        assert_eq!(Decibels::from_full_scale(0.1), Decibels(-10.))
    }
}