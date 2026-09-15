use crate::Action;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub chord: Vec<u8>,
    pub profile: String,
    pub action: Action,
}
