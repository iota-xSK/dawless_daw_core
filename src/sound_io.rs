use cpal::{
    traits::{DeviceTrait, HostTrait},
    StreamConfig,
};
use heapless::spsc::Producer;

use crate::{Data, Gadget, InputCtx, PaintBrush};

pub struct Dac<'a> {
    producer: Producer<'a, f32, 64>,
    config: StreamConfig,
}

impl<P: PaintBrush, I: InputCtx> Gadget<32, 32, 32, 32, 64, P, I> for Dac<'_> {
    fn draw(&mut self, _: P) {
        todo!()
    }

    fn process(
        &mut self,
        signal_in: [Option<&[f32]>; 32],
        _: [Option<&crate::Data>; 32],
        _: f32,
        sample_n: usize,
    ) -> crate::Output<32, 32, 64> {
        while !self.producer.ready() {
        }
        if let Some(buff) = signal_in[0] {
            for i in 0..sample_n {
                if self.producer.ready() {
                    let _ = self.producer.enqueue(buff[i]);
                }
            }
        }

        crate::Output { data: [None; 32], signals: [[0.0; 64]; 32] }
    }

    fn input(&mut self, _: I) {
        todo!()
    }
}
