use super::input::{Input, InputError};
use super::output::{Output, OutputError};
use super::Frame;

/// A processing step that transforms an input into an output
pub trait Step {
    type Input;
    type Output;

    /// Process the next input item.
    /// After this is called, `pop_output` **must** be called until it returns
    /// `None` before another input is pushed.
    /// i.e. a Step shall not be required to buffer more than one input
    fn push_input(&mut self, input: Self::Input);

    /// Get the next output of this step, if available.
    fn pop_output(&mut self) -> Option<Self::Output>;
}

/// Encapsulates some audio input, a processing step to transform that input,
/// and an output to sink the results.
/// The processing step is generally expected to be a `Chain`
pub struct Pipeline<I: Input, S: Step<Input = I::Item, Output = Frame>, O: Output> {
    input: I,
    step: S,
    output: O,
}

#[derive(Debug)]
pub enum ProcessError {
    InputError(InputError),
    OutputError(OutputError),
}

impl<I: Input, S: Step<Input = I::Item, Output = Frame>, O: Output> Pipeline<I, S, O> {
    pub fn new(input: I, step: S, output: O) -> Pipeline<I, S, O> {
        Pipeline {
            input,
            step,
            output,
        }
    }

    pub fn process_once(&mut self) -> Result<(), ProcessError> {
        match self.input.read() {
            Ok(frame) => {
                self.step.push_input(frame);
                while let Some(output) = self.step.pop_output() {
                    if let Err(e) = self.output.push(output) {
                        return Err(ProcessError::OutputError(e));
                    }
                }
                Ok(())
            }
            Err(e) => Err(ProcessError::InputError(e)),
        }
    }
}

/// A `Step` that outputs its input.
/// Which is "useful" if you want a `Pipeline` that just copies its input to
/// its output.
pub struct Identity<T> {
    next: Option<T>,
}

impl<T> Identity<T> {
    pub fn new() -> Identity<T> {
        Identity { next: None }
    }
}

impl<T> Step for Identity<T> {
    type Input = T;
    type Output = T;

    fn push_input(&mut self, input: T) {
        assert!(self.next.is_none());
        self.next = Some(input);
    }

    fn pop_output(&mut self) -> Option<T> {
        self.next.take()
    }
}

/// Connects the output of one `Step` to the input of another.
/// This can be used to compose a multistep processing pipeline.
pub struct Chain<First: Step, Second: Step<Input = First::Output>> {
    first: First,
    second: Second,
}

impl<First: Step, Second: Step<Input = First::Output>> Step for Chain<First, Second> {
    type Input = First::Input;
    type Output = Second::Output;

    fn push_input(&mut self, input: Self::Input) {
        self.first.push_input(input);
    }

    fn pop_output(&mut self) -> Option<Self::Output> {
        if let Some(output) = self.second.pop_output() {
            Some(output)
        } else {
            while let Some(intermediate) = self.first.pop_output() {
                self.second.push_input(intermediate);
                if let Some(output) = self.second.pop_output() {
                    return Some(output);
                }
            }
            None
        }
    }
}
