use std::array::{self, from_fn};

use arrayvec::ArrayVec;
use heapless::{Deque, FnvIndexMap};

pub mod sound_io;
pub use sound_io::*;

// TODO: think of what drawing features it actually needs.
pub trait PaintBrush {
    fn draw_rectangle(&mut self, x: u32, y: u32, w: u32, h: u32);
    fn draw_text(&mut self, text: &str);
    fn delta_tme(&self) -> f64;
}

// TODO: think of what input features it actually needs.
pub trait InputCtx {
    fn mouse_pressed(&mut self);
    fn get_key(&mut self) -> u8;
}

#[derive(Clone, Copy)]
pub enum Data {
    Float(f32),
    Int(i32),
}

pub trait Gadget<
    const MAX_SIGNAL_IN: usize,
    const MAX_SIGNAL_OUT: usize,
    const MAX_DATA_IN: usize,
    const MAX_DATA_OUT: usize,
    const MIN_BUFF_SIZE: usize,
    P: PaintBrush,
    I: InputCtx,
>: Send
{
    fn draw(&mut self, brush: P);
    fn process(
        &mut self,
        signal_in: [Option<&[f32]>; MAX_SIGNAL_IN],
        data_in: [Option<&Data>; MAX_DATA_IN],
        sr: u32,
        sample_n: usize,
    ) -> Output<MAX_SIGNAL_OUT, MAX_DATA_OUT, MIN_BUFF_SIZE>;
    fn input(&mut self, inpu_ctx: I);
}

struct Node<
    const MAX_SIGNAL_IN: usize,
    const MAX_SIGNAL_OUT: usize,
    const MAX_DATA_IN: usize,
    const MAX_DATA_OUT: usize,
    const MIN_BUFF_SIZE: usize,
    P: PaintBrush,
    I: InputCtx,
> {
    gadget: Box<
        dyn Gadget<MAX_SIGNAL_IN, MAX_SIGNAL_OUT, MAX_DATA_IN, MAX_DATA_OUT, MIN_BUFF_SIZE, P, I>,
    >,
    signal_inputs: [Option<(usize, usize)>; MAX_SIGNAL_IN],
    data_inputs: [Option<(usize, usize)>; MAX_DATA_IN],
}

#[derive(Clone, Copy)]
pub struct Output<
    const MAX_SIGNAL_OUT: usize,
    const MAX_DATA_OUT: usize,
    const MIN_BUFF_SIZE: usize,
> {
    pub data: [Option<Data>; MAX_DATA_OUT],
    pub signals: [[f32; MIN_BUFF_SIZE]; MAX_SIGNAL_OUT],
}

pub struct ConnectionGraph<
    const MAX_SIGNAL_IN: usize,
    const MAX_SIGNAL_OUT: usize,
    const MAX_DATA_IN: usize,
    const MAX_DATA_OUT: usize,
    const MIN_BUFF_SIZE: usize,
    P: PaintBrush,
    I: InputCtx,
    const MAX_NODES: usize,
> {
    outputs: [Output<MAX_SIGNAL_OUT, MAX_DATA_OUT, MIN_BUFF_SIZE>; MAX_NODES],
    nodes: [Option<
        Node<MAX_SIGNAL_IN, MAX_SIGNAL_OUT, MAX_DATA_IN, MAX_DATA_OUT, MIN_BUFF_SIZE, P, I>,
    >; MAX_NODES],
    execution_order: ArrayVec<usize, MAX_NODES>,
}

pub struct Handle(usize);

const MAX_SIGNAL_IN: usize = 32;
const MAX_SIGNAL_OUT: usize = 32;
const MAX_DATA_IN: usize = 32;
const MAX_DATA_OUT: usize = 32;
const MAX_NODES: usize = 64;
impl<const MIN_BUFF_SIZE: usize, P: PaintBrush, I: InputCtx>
    ConnectionGraph<
        MAX_SIGNAL_IN,
        MAX_SIGNAL_OUT,
        MAX_DATA_IN,
        MAX_DATA_OUT,
        MIN_BUFF_SIZE,
        P,
        I,
        MAX_NODES,
    >
{
    pub fn process(&mut self, sr: u32, sample_n: usize) {
        let mut sample_n = sample_n as isize;
        while sample_n > 0 {
            for i in 0..MAX_NODES {
                if let Some(ref mut node) = self.nodes[i] {
                    let mut signal_inputs: [Option<&[f32]>; MAX_SIGNAL_IN] =
                        array::from_fn(|_| None);

                    for (j, input) in node.signal_inputs.iter().enumerate() {
                        if let Some((in_node, in_port)) = input {
                            signal_inputs[j] = Some(
                                &self.outputs[*in_node].signals[*in_port]
                                    [0..(sample_n.min(MIN_BUFF_SIZE as isize) as usize)],
                            );
                        }
                    }

                    let mut data_inputs: [Option<&Data>; MAX_DATA_IN] = array::from_fn(|_| None);

                    for (j, input) in node.data_inputs.iter().enumerate() {
                        if let Some((in_node, in_port)) = input {
                            data_inputs[j] = self.outputs[*in_node].data[*in_port].as_ref();
                        }
                    }
                    self.outputs[i] = node.gadget.process(
                        signal_inputs,
                        data_inputs,
                        sr,
                        (sample_n as usize).min(MIN_BUFF_SIZE),
                    );
                }
            }
            sample_n -= MIN_BUFF_SIZE as isize;
        }
    }
    pub fn add_node(
        &mut self,
        gadget: Box<
            dyn Gadget<
                MAX_SIGNAL_IN,
                MAX_SIGNAL_OUT,
                MAX_DATA_IN,
                MAX_DATA_OUT,
                MIN_BUFF_SIZE,
                P,
                I,
            >,
        >,
    ) -> Result<
        Handle,
        Box<
            dyn Gadget<
                MAX_SIGNAL_IN,
                MAX_SIGNAL_OUT,
                MAX_DATA_IN,
                MAX_DATA_OUT,
                MIN_BUFF_SIZE,
                P,
                I,
            >,
        >,
    > {
        for j in 0..MAX_NODES {
            if let None = self.nodes[j] {
                self.nodes[j] = Some(Node {
                    gadget,
                    signal_inputs: [None; MAX_SIGNAL_IN],
                    data_inputs: [None; MAX_DATA_IN],
                });
                return Ok(Handle(j));
            }
        }
        return Err(gadget);
    }
    pub fn add_data_edge(
        &mut self,
        from_handle: &Handle,
        from_input: usize,
        to_handle: &Handle,
        to_input: usize,
    ) -> bool {
        self.nodes[to_handle.0].as_mut().unwrap().data_inputs[to_input] =
            Some((from_handle.0, from_input));

        let old_order = self.execution_order.clone();
        self.execution_order.clear();

        // toposort: kahn's algorithm

        const INPUTLEN: usize = MAX_SIGNAL_IN + MAX_DATA_IN;
        let mut kahn_graph: FnvIndexMap<_, _, MAX_NODES> = FnvIndexMap::new();
        for i in 0..MAX_NODES {
            if let Some(ref mut node) = self.nodes[i] {
                let mut edges: ArrayVec<usize, INPUTLEN> = ArrayVec::new();
                for i in 0..MAX_DATA_IN {
                    if let Some((node, _)) = node.data_inputs[i] {
                        edges.push(node);
                    }
                }
                for i in 0..MAX_SIGNAL_IN {
                    if let Some((node, _)) = node.signal_inputs[i] {
                        edges.push(node);
                    }
                }
                let _ = kahn_graph.insert(i, edges);
            }
        }

        let mut s: Deque<usize, MAX_NODES> = Deque::new();
        for i in 0..MAX_NODES {
            if let Some(ref node) = self.nodes[i] {
                if node.data_inputs == [None; MAX_DATA_IN]
                    && node.signal_inputs == [None; MAX_SIGNAL_IN]
                {
                    let _ = s.push_back(i);
                    kahn_graph.remove(&i);
                }
            }
        }

        while !s.is_empty() {
            let n = s.pop_front().unwrap();
            self.execution_order.push(n);

            kahn_graph.retain(|m, edges| {
                edges.retain(|i| *i != n);
                let is_empty = edges.is_empty();
                if is_empty {
                    let _ = s.push_back(*m);
                }
                !is_empty
            });
        }

        for (_, node) in kahn_graph {
            if !node.is_empty() {
                self.execution_order = old_order;
                self.nodes[to_handle.0].as_mut().unwrap().data_inputs[to_input] = None;
                return false;
            }
        }

        return true;
    }

    pub fn add_signal_edge(
        &mut self,
        from_handle: &Handle,
        from_input: usize,
        to_handle: &Handle,
        to_input: usize,
    ) -> bool {
        self.nodes[to_handle.0].as_mut().unwrap().signal_inputs[to_input] =
            Some((from_handle.0, from_input));

        let old_order = self.execution_order.clone();
        self.execution_order.clear();

        // toposort: kahn's algorithm

        const INPUTLEN: usize = MAX_SIGNAL_IN + MAX_DATA_IN;
        let mut kahn_graph: FnvIndexMap<_, _, MAX_NODES> = FnvIndexMap::new();
        for i in 0..MAX_NODES {
            if let Some(ref mut node) = self.nodes[i] {
                let mut edges: ArrayVec<usize, INPUTLEN> = ArrayVec::new();
                for i in 0..MAX_DATA_IN {
                    if let Some((node, _)) = node.data_inputs[i] {
                        edges.push(node);
                    }
                }
                for i in 0..MAX_SIGNAL_IN {
                    if let Some((node, _)) = node.signal_inputs[i] {
                        edges.push(node);
                    }
                }
                let _ = kahn_graph.insert(i, edges);
            }
        }

        let mut s: Deque<usize, MAX_NODES> = Deque::new();
        for i in 0..MAX_NODES {
            if let Some(ref node) = self.nodes[i] {
                if node.data_inputs == [None; MAX_DATA_IN]
                    && node.signal_inputs == [None; MAX_SIGNAL_IN]
                {
                    let _ = s.push_back(i);
                    kahn_graph.remove(&i);
                }
            }
        }

        while !s.is_empty() {
            let n = s.pop_front().unwrap();
            self.execution_order.push(n);

            kahn_graph.retain(|m, edges| {
                edges.retain(|i| *i != n);
                let is_empty = edges.is_empty();
                if is_empty {
                    let _ = s.push_back(*m);
                }
                !is_empty
            });
        }

        for (_, node) in kahn_graph {
            if !node.is_empty() {
                self.execution_order = old_order;
                self.nodes[to_handle.0].as_mut().unwrap().signal_inputs[to_input] = None;
                return false;
            }
        }

        return true;
    }

    pub fn new() -> Self {
        Self {
            outputs: [Output {
                data: [None; MAX_DATA_OUT],
                signals: [[0.0; MIN_BUFF_SIZE]; MAX_SIGNAL_OUT],
            }; MAX_NODES],
            nodes: from_fn(|_| None),
            execution_order: ArrayVec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;

    impl PaintBrush for Dummy {
        fn draw_rectangle(&mut self, _: u32, _: u32, _: u32, _: u32) {}

        fn draw_text(&mut self, _: &str) {}

        fn delta_tme(&self) -> f64 {
            0.0
        }
    }
    impl InputCtx for Dummy {
        fn mouse_pressed(&mut self) {}

        fn get_key(&mut self) -> u8 {
            0
        }
    }

    impl<const MIN_BUFF_SIZE: usize>
        Gadget<
            MAX_SIGNAL_IN,
            MAX_SIGNAL_OUT,
            MAX_DATA_IN,
            MAX_DATA_OUT,
            MIN_BUFF_SIZE,
            Dummy,
            Dummy,
        > for Dummy
    {
        fn draw(&mut self, _: Dummy) {}

        fn process(
            &mut self,
            _: [Option<&[f32]>; MAX_SIGNAL_IN],
            _: [Option<&Data>; MAX_DATA_IN],
            _: u32,
            _: usize,
        ) -> Output<MAX_SIGNAL_OUT, MAX_DATA_OUT, MIN_BUFF_SIZE> {
            todo!()
        }

        fn input(&mut self, _: Dummy) {
            todo!()
        }
    }

    #[test]
    fn toposort_success() {
        let mut graph: ConnectionGraph<32, 32, 32, 32, 32, Dummy, Dummy, 64> =
            ConnectionGraph::new();

        let a = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();
        let b = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();
        let c = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();
        let d = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();
        let e = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();
        let f = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();
        let g = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();

        assert!(
            graph.add_data_edge(&a, 0, &c, 0)
                && graph.add_signal_edge(&a, 0, &d, 0)
                && graph.add_data_edge(&b, 0, &e, 0)
                && graph.add_signal_edge(&c, 0, &f, 0)
                && graph.add_data_edge(&d, 0, &g, 0)
                && graph.add_data_edge(&e, 0, &g, 0)
        );
        println!("{:?}", graph.execution_order);
    }

    #[test]
    fn toposort_fail() {
        let mut graph: ConnectionGraph<32, 32, 32, 32, 32, Dummy, Dummy, 64> =
            ConnectionGraph::new();

        let a = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();
        let b = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();
        let c = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();
        let d = graph
            .add_node(Box::new(Dummy))
            .map_err(|_| "something went horribly wrong")
            .unwrap();

        assert!(
            !(graph.add_data_edge(&a, 0, &c, 0)
                && graph.add_data_edge(&c, 0, &d, 0)
                && graph.add_signal_edge(&d, 0, &a, 0))
                && graph.add_data_edge(&b, 0, &c, 0)
        );
        println!("{:?}", graph.execution_order);
    }
}
