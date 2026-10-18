use super::*;
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sent {
    Ack,
    Error(String),
    Screen(Vec<u8>),
    Attached,
    Retire,
}

impl From<&Response> for Sent {
    fn from(response: &Response) -> Self {
        match response {
            Response::Ack {} => Sent::Ack,
            Response::Error { message } => Sent::Error(message.clone()),
            Response::Screen { bytes } => Sent::Screen(bytes.clone()),
            Response::Attached {} => Sent::Attached,
            Response::Retire {} => Sent::Retire,
        }
    }
}

#[derive(Default)]
struct State {
    incoming: VecDeque<Request>,
    sent: Vec<Sent>,
    closed: bool,
}

#[derive(Clone, Default)]
pub struct FakeConnection(Rc<RefCell<State>>);

impl FakeConnection {
    pub fn push(&self, request: Request) {
        self.0.borrow_mut().incoming.push_back(request);
    }
    pub fn close(&self) {
        self.0.borrow_mut().closed = true;
    }
    pub fn sent(&self) -> Vec<Sent> {
        self.0.borrow().sent.clone()
    }
    pub fn take_sent(&self) -> Vec<Sent> {
        std::mem::take(&mut self.0.borrow_mut().sent)
    }
}

impl Connection for FakeConnection {
    fn read(&mut self) -> std::io::Result<Option<Vec<Request>>> {
        let mut state = self.0.borrow_mut();
        if state.closed {
            return Ok(None);
        }
        Ok(Some(state.incoming.drain(..).collect()))
    }
    fn send(&mut self, response: &Response) -> std::io::Result<()> {
        self.0.borrow_mut().sent.push(Sent::from(response));
        Ok(())
    }
    fn flush(&mut self) -> std::io::Result<bool> {
        Ok(true)
    }
    fn is_idle(&self) -> bool {
        true
    }
}
