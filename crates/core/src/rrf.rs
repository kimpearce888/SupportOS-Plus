pub const RRF_K: u32 = 60;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_constant_is_pinned() {
        assert_eq!(RRF_K, 60);
    }
}
