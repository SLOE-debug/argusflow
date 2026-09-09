use super::*;
#[test]
fn blank_separates_repeated_characters_and_scores_only_kept_steps() {
    let dictionary = vec![String::new(), "中".into(), " ".into()];
    let values = [
        0.1, 0.9, 0.0, 0.1, 0.8, 0.1, 1.0, 0.0, 0.0, 0.1, 0.9, 0.0, 0.1, 0.1, 0.8,
    ];
    let (text, confidence) = ctc(&values, 5, 3, &dictionary).unwrap();
    assert_eq!(text, "中中 ");
    assert!((confidence - 2.6 / 3.0).abs() < 0.0001);
}
