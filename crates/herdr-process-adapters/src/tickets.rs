use crate::unix;
use herdr_process_application::Tickets;

pub struct RandomTickets;

impl Tickets for RandomTickets {
    fn issue(&self) -> String {
        unix::random_hex(32)
    }
}
