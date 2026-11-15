use super::*;
use crate::testsupport::fixture;

#[test]
fn api_concurrency_save_reaches_the_live_provider() {
    let db = fixture(Vec::new());
    let real = std::sync::Arc::new(spp_core::helpscout_real::RealHelpScoutProvider::new(
        db.clone(),
        spp_core::helpscout_real::HsCredentials::default(),
    ));
    assert_eq!(real.api_concurrency(), 2, "unset: the default");

    let mut page = SettingsPage::new();
    page.set_provider_handle(std::sync::Arc::new(
        spp_core::provider_handle::ProviderHandle::new_real(std::sync::Arc::clone(&real)),
    ));

    page.reload(&db);
    page.general.as_mut().expect("loaded").api_concurrency = 6;
    page.save_general(&db);

    assert!(
        page.error.is_none(),
        "the save must succeed: {:?}",
        page.error
    );
    assert_eq!(
        real.api_concurrency(),
        6,
        "the live provider's queues resized in-session"
    );
    let rebuilt = spp_core::helpscout_real::RealHelpScoutProvider::new(
        db.clone(),
        spp_core::helpscout_real::HsCredentials::default(),
    );
    assert_eq!(
        rebuilt.api_concurrency(),
        6,
        "construction applies the stored setting"
    );
}
