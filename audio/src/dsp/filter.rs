use std::collections::VecDeque;

use crate::stream::pipeline::Step;

/// Implements a linear constant-coefficient difference equation, which can
/// represent any linear, time-invariant discrete system
pub struct LTI {
    feedback: Vec<f32>,     // often denoted a[n]. a[0] must be 1.0
    feedforward: Vec<f32>,  // often denoted b[n]
    inputs: VecDeque<f32>,  // front / 0 is most recent
    outputs: VecDeque<f32>, // front / 0 is most recent
    next: isize,            // index of the next output value, in outputs
}

impl LTI {
    pub fn new(feedback: Vec<f32>, feedforward: Vec<f32>) -> LTI {
        assert_eq!(feedback[0], 1.0);
        assert!(feedforward.len() > 0);
        let mut inputs = VecDeque::with_capacity(feedforward.len());
        inputs.resize(feedforward.len(), 0.0);
        let mut outputs = VecDeque::with_capacity(feedback.len());
        outputs.resize(feedback.len(), 0.0);
        LTI {
            feedback,
            feedforward,
            inputs,
            outputs,
            next: -1,
        }
    }

    pub fn reset(&mut self) {
        for i in &mut self.inputs {
            *i = 0.;
        }
        for i in &mut self.outputs {
            *i = 0.;
        }
        self.next = -1;
    }
}

impl Step<'_> for LTI {
    type Input = f32;
    type Output = f32;
    type Result = Option<f32>;

    fn process(&mut self, input: f32) -> Option<f32> {
        // Add the new input to the input ringbuffer and multiply inputs with
        // feedforward coefficients
        self.inputs.pop_back();
        self.inputs.push_front(input);
        let mut next_out = 0f32;
        for i in 0..self.feedforward.len() {
            next_out += self.feedforward[i] * self.inputs[i];
        }

        // Make space for the new output in the output ringbuffer, then
        // multiply outputs with feedback coefficients
        self.outputs.pop_back();
        self.outputs.push_front(0f32);
        self.next += 1;
        // Note that since the front of outputs is 0 (we're carrying the result
        // of feedforward in next_out instead) we can skip the first index here:
        for i in 1..self.feedback.len() {
            next_out -= self.feedback[i] * self.outputs[i];
        }

        self.outputs[0] = next_out;
        Some(next_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_response(lti: &mut LTI, input: &[f32], expect_output: &[f32]) {
        assert_eq!(input.len(), expect_output.len());
        let mut output = Vec::new();
        for i in 0..input.len() {
            output.push(lti.process(input[i]).unwrap());
        }
        assert_eq!(expect_output, &output);
    }

    #[test]
    fn test_feedforward() {
        let mut lti = LTI::new(vec![1.], vec![0.5, 0.0, 0.3]);
        assert_response(&mut lti, &[1., 0., 0.], &[0.5, 0.0, 0.3]);
        assert_response(&mut lti, &[1., 0., 1.], &[0.5, 0.0, 0.8]);
    }

    #[test]
    fn test_feedback() {
        let mut lti = LTI::new(vec![1., 0., -0.5, -0.2], vec![1.0]);
        assert_response(
            &mut lti,
            &[1., 0., 0.0, 0.0, 0.00, 0.00, 0.000],
            &[1., 0., 0.5, 0.2, 0.25, 0.20, 0.165],
        );
    }
}
