//! Random documents and queries must produce the same results as the last
//! release. Set `SCAH_DIFF_CASES` to run more cases and `SCAH_DIFF_SEED` to
//! explore a different sequence.

use scah_differential::{Case, Rng, run_current, run_reference};

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

#[test]
fn current_engine_matches_last_release() {
    let cases = env_u64("SCAH_DIFF_CASES", 2_000);
    let seed = env_u64("SCAH_DIFF_SEED", 0x5ca4);
    let mut compared = 0;
    for index in 0..cases {
        let case = Case::generate(&mut Rng::new(seed.wrapping_add(index)));
        let current = run_current(&case);
        let reference = run_reference(&case);
        assert_eq!(
            current.is_some(),
            reference.is_some(),
            "selector acceptance differs for case {index}\n{}",
            case.describe()
        );
        if let (Some(current), Some(reference)) = (current, reference) {
            assert_eq!(
                current,
                reference,
                "results differ for case {index} (seed {seed})\n{}",
                case.describe()
            );
            compared += 1;
        }
    }
    assert!(
        compared * 2 > cases,
        "too few valid cases: {compared} of {cases}"
    );
}
