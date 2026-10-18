use super::*;
use herdr_process_domain::Command;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

pub struct Fakes;

impl Ports for Fakes {
    type Host = FakeHost;
    type Call = FakeCall;
    type Processes = FakeProcesses;
    type Process = FakeProcess;
    type Tickets = FakeTickets;
    type Screen = FakeScreen;
    type Connection = FakeConnection;
}

#[derive(Clone, Default)]
pub struct FakeHost {
    pub opened: Rc<RefCell<Vec<OpenView>>>,
    pub focused: Rc<RefCell<Vec<String>>>,
    replies: Rc<RefCell<VecDeque<Result<HostReply, HostFailure>>>>,
}

impl FakeHost {
    pub fn reply(&self, reply: Result<HostReply, HostFailure>) {
        self.replies.borrow_mut().push_back(reply);
    }
    pub fn opened(&self) -> Vec<OpenView> {
        self.opened.borrow().clone()
    }
    fn call(&self) -> FakeCall {
        FakeCall(self.replies.borrow_mut().pop_front())
    }
}

impl Host for FakeHost {
    type Call = FakeCall;
    fn open(&self, view: &OpenView) -> anyhow::Result<FakeCall> {
        self.opened.borrow_mut().push(view.clone());
        Ok(self.call())
    }
    fn focus(&self, pane: &str) -> anyhow::Result<FakeCall> {
        self.focused.borrow_mut().push(pane.into());
        Ok(self.call())
    }
}

pub struct FakeCall(Option<Result<HostReply, HostFailure>>);

impl Call for FakeCall {
    fn poll(&mut self) -> Result<Option<HostReply>, HostFailure> {
        match self.0.clone() {
            None => Ok(None),
            Some(Ok(reply)) => Ok(Some(reply)),
            Some(Err(failure)) => Err(failure),
        }
    }
}

#[derive(Clone, Default)]
pub struct FakeProcesses {
    pub spawned: Rc<RefCell<Vec<(Command, FakeProcess)>>>,
}

impl FakeProcesses {
    pub fn commands(&self) -> Vec<Command> {
        self.spawned.borrow().iter().map(|s| s.0.clone()).collect()
    }
    pub fn process(&self, index: usize) -> FakeProcess {
        self.spawned.borrow()[index].1.clone()
    }
}

impl Processes for FakeProcesses {
    type Process = FakeProcess;
    fn spawn(&self, command: &Command, rows: u16, cols: u16) -> anyhow::Result<FakeProcess> {
        let process = FakeProcess::default();
        process.0.borrow_mut().size = (rows, cols);
        self.spawned
            .borrow_mut()
            .push((command.clone(), process.clone()));
        Ok(process)
    }
}

#[derive(Default)]
pub struct ProcessState {
    pub written: Vec<u8>,
    pub output: VecDeque<Vec<u8>>,
    pub exit: Option<i32>,
    pub terminated: bool,
    pub size: (u16, u16),
}

#[derive(Clone, Default)]
pub struct FakeProcess(Rc<RefCell<ProcessState>>);

impl FakeProcess {
    pub fn prints(&self, bytes: &[u8]) {
        self.0.borrow_mut().output.push_back(bytes.to_vec());
    }
    pub fn exits(&self, code: i32) {
        self.0.borrow_mut().exit = Some(code);
    }
    pub fn written(&self) -> Vec<u8> {
        self.0.borrow().written.clone()
    }
    pub fn terminated(&self) -> bool {
        self.0.borrow().terminated
    }
    pub fn size(&self) -> (u16, u16) {
        self.0.borrow().size
    }
}

impl Process for FakeProcess {
    fn write(&mut self, bytes: &[u8]) -> anyhow::Result<usize> {
        self.0.borrow_mut().written.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn read_available(&mut self) -> anyhow::Result<Vec<u8>> {
        Ok(self.0.borrow_mut().output.pop_front().unwrap_or_default())
    }
    fn poll_exit(&mut self) -> anyhow::Result<Option<i32>> {
        Ok(self.0.borrow().exit)
    }
    fn resize(&mut self, rows: u16, cols: u16) -> anyhow::Result<()> {
        self.0.borrow_mut().size = (rows, cols);
        Ok(())
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.0.borrow_mut().terminated = true;
        Ok(())
    }
}

#[derive(Default)]
pub struct FakeTickets(Cell<u32>);

impl Tickets for FakeTickets {
    fn issue(&self) -> String {
        self.0.set(self.0.get() + 1);
        format!("ticket-{}", self.0.get())
    }
}

pub struct FakeScreen {
    bytes: Vec<u8>,
}

impl Screen for FakeScreen {
    fn new(_rows: u16, _cols: u16) -> Self {
        Self { bytes: Vec::new() }
    }
    fn feed(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    fn resize(&mut self, _rows: u16, _cols: u16) {}
    fn snapshot(&self) -> Vec<u8> {
        self.bytes.clone()
    }
    fn take_responses(&mut self) -> Vec<u8> {
        Vec::new()
    }
}
