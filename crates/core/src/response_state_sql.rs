use crate::catalog::ResponseState;
pub const RESPONSE_STATE_SQL: &str = "response_state";

pub fn filter_by_response_state(state: ResponseState) -> String {
    format!("response_state = '{}'", state.as_str())
}
