use super::input::{Input, InputError};
use super::output::{Output, OutputError};
use super::Frame;
use std::marker::PhantomData;

/// A processing step that transforms an input into an output
pub trait Step<'a> {
    type Input;
    type Output;
    type Result: IntoIterator<Item = Self::Output>;

    /// Process the next input item.
    fn process(&'a mut self, input: Self::Input) -> Self::Result;
}

/// Encapsulates some audio input, a processing step to transform that input,
/// and an output to sink the results.
/// The processing step is generally expected to be a `Chain`
pub struct Pipeline<I, S, O>
where
    I: Input,
    for<'a> S: Step<'a, Input = I::Item, Output = Frame>,
    O: Output,
{
    input: I,
    step: S,
    output: O,
}

#[derive(Debug)]
pub enum ProcessError {
    InputError(InputError),
    OutputError(OutputError),
}

impl<I, S, O> Pipeline<I, S, O>
where
    I: Input,
    for<'a> S: Step<'a, Input = I::Item, Output = Frame>,
    O: Output,
{
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
                for output in self.step.process(frame).into_iter() {
                    if let Err(e) = self.output.push(output) {
                        return Err(ProcessError::OutputError(e));
                    }
                }
                Ok(())
            }
            Err(e) => Err(ProcessError::InputError(e)),
        }
    }

    pub fn input_mut(&mut self) -> &mut I {
        &mut self.input
    }

    pub fn step_mut(&mut self) -> &mut S {
        &mut self.step
    }
}

/// A `Step` that outputs its input.
/// Which is "useful" if you want a `Pipeline` that just copies its input to
/// its output.
pub struct Identity<T>(PhantomData<T>);

impl<T> Identity<T> {
    pub fn new() -> Identity<T> {
        Identity(PhantomData)
    }
}

impl<T> Step<'_> for Identity<T> {
    type Input = T;
    type Output = T;
    type Result = Option<T>;

    fn process(&mut self, input: T) -> Option<T> {
        Some(input)
    }
}

/// Connects the output of one `Step` to the input of another.
/// This can be used to compose a multistep processing pipeline.
pub struct Chain<'a, First, Second>
where
    First: Step<'a>,
    Second: Step<'a, Input = First::Output>,
{
    first: First,
    second: Second,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, First, Second> Chain<'a, First, Second>
where
    First: Step<'a>,
    Second: Step<'a, Input = First::Output>,
{
    pub fn new(first: First, second: Second) -> Chain<'a, First, Second> {
        Chain {
            first,
            second,
            _phantom: PhantomData,
        }
    }

    pub fn first_mut(&mut self) -> &mut First {
        &mut self.first
    }

    pub fn second_mut(&mut self) -> &mut Second {
        &mut self.second
    }
}

pub struct ChainResult<'a, IntermediateIter, SecondStep>
where
    SecondStep: Step<'a>,
    IntermediateIter: Iterator<Item = SecondStep::Input>,
{
    first_res: IntermediateIter,
    second_step: &'a mut SecondStep,
    second_res: Option<<<SecondStep as Step<'a>>::Result as IntoIterator>::IntoIter>,
}

impl<'a, IntermediateIter, SecondStep> Iterator for ChainResult<'a, IntermediateIter, SecondStep>
where
    SecondStep: Step<'a>,
    IntermediateIter: Iterator<Item = SecondStep::Input>,
{
    type Item = SecondStep::Output;
    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        todo!()
    }
    // fn push_input(&mut self, input: Self::Input) {
    //     self.first.push_input(input);
    // }

    // fn pop_output(&mut self) -> Option<Self::Output> {
    //     if let Some(output) = self.second.pop_output() {
    //         Some(output)
    //     } else {
    //         while let Some(intermediate) = self.first.pop_output() {
    //             self.second.push_input(intermediate);
    //             if let Some(output) = self.second.pop_output() {
    //                 return Some(output);
    //             }
    //         }
    //         None
    //     }
    // }
}

impl<'a, First, Second> Step<'a> for Chain<'a, First, Second>
where
    First: Step<'a> + 'a,
    Second: Step<'a, Input = First::Output> + 'a,
{
    type Input = First::Input;
    type Output = Second::Output;
    type Result = ChainResult<'a, <<First as Step<'a>>::Result as IntoIterator>::IntoIter, Second>;

    fn process(&'a mut self, input: Self::Input) -> Self::Result {
        let first_res = self.first.process(input).into_iter();
        ChainResult {
            first_res,
            second_step: &mut self.second,
            second_res: None,
        }
    }
}
