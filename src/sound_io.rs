pub use crossbeam_channel::{bounded, Receiver, Sender};

use crate::{Gadget, InputCtx, Output, PaintBrush};

pub struct Dac {
    l_channel: Sender<f32>,
    r_channel: Sender<f32>,
}

impl<P: PaintBrush, I: InputCtx> Gadget<32, 32, 32, 32, 64, P, I> for Dac {
    fn draw(&mut self, _: P) {
        todo!()
    }

    fn process(
        &mut self,
        signal_in: [Option<&[f32]>; 32],
        _: [Option<&crate::Data>; 32],
        _: u32,
        sample_n: usize,
    ) -> crate::Output<32, 32, 64> {
        if let Some(buff) = signal_in[0] {
            for i in 0..sample_n {
                let _ = self.l_channel.send(buff[i]);
            }
        }


        if let Some(buff) = signal_in[1] {
            for i in 0..sample_n {
                let _ = self.r_channel.send(buff[i]);
            }
        }



        Output { data: [None; 32], signals: [[0.0; 64]; 32] }
    }

    fn input(&mut self, _: I) {
        todo!()
    }
}

pub struct Adc {
    l_channel: Receiver<f32>,
    r_channel: Receiver<f32>
}

impl<P: PaintBrush, I: InputCtx> Gadget<32, 32, 32, 32, 64, P, I> for Adc {
    fn draw(&mut self, _: P) {
        todo!()
    }

    fn process(
        &mut self,
        _: [Option<&[f32]>; 32],
        _: [Option<&crate::Data>; 32],
        _: u32,
        sample_n: usize,
    ) -> crate::Output<32, 32, 64> {
        let mut output = Output { data: [None; 32], signals: [[0.0; 64]; 32] };
        for i in 0..sample_n {
            output.signals[0][i] = self.l_channel.recv().unwrap_or(0.0);
        }

        for i in 0..sample_n {
            output.signals[1][i] = self.r_channel.recv().unwrap_or(0.0);
        }
        output
    }

    fn input(&mut self, _: I) {
        todo!()
    }
}

impl Adc {
    pub fn new() -> (Sender<f32>, Sender<f32>, Adc){
        let (l_snd, l_rcv) = bounded(64);
        let (r_snd, r_rcv) = bounded(64);
        (l_snd, r_snd,  Adc{ l_channel: l_rcv, r_channel: r_rcv })
    }
}


impl Dac {
    pub fn new() -> (Receiver<f32>, Receiver<f32>, Dac){
        let (l_snd, l_rcv) = bounded(64);
        let (r_snd, r_rcv) = bounded(64);
        (l_rcv, r_rcv,  Dac{ l_channel: l_snd, r_channel: r_snd })
    }
}
