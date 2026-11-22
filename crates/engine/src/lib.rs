//! Checks answers and runs a practice session.
//!
//! [`check`] compares one typed answer with an [`AnswerSpec`]. [`Practice`] walks a learner
//! through a skill: the current problem, what has been typed, the result of the last check,
//! how many worked steps are showing, and the score.

use i_success_model::{AnswerSpec, Problem, Skill};

/// The result of checking one answer box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Verdict {
    /// Nothing was typed.
    Empty,
    Correct,
    Wrong,
}

/// Reads a typed number. Accepts spaces and thousands commas (`1,250`), a leading `$`, and the
/// true minus sign `−` as well as `-`. `None` when the text is not a number.
pub fn parse_number(text: &str) -> Option<f64> {
    let cleaned: String = text
        .trim()
        .trim_start_matches('$')
        .chars()
        .filter(|c| *c != ',' && !c.is_whitespace())
        .map(|c| if c == '\u{2212}' { '-' } else { c })
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Checks one typed answer. A numeric expected answer is compared as a number, within the
/// box's tolerance; any other answer is compared as text, ignoring case and outer spaces.
pub fn check(spec: &AnswerSpec, typed: &str) -> Verdict {
    if typed.trim().is_empty() {
        return Verdict::Empty;
    }
    let right = match parse_number(&spec.answer) {
        Some(expected) => parse_number(typed).is_some_and(|got| (got - expected).abs() <= spec.tol.max(0.0) + f64::EPSILON * expected.abs().max(1.0)),
        None => typed.trim().eq_ignore_ascii_case(spec.answer.trim()),
    };
    if right { Verdict::Correct } else { Verdict::Wrong }
}

/// A learner working through one skill.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Practice {
    skill: String,
    index: usize,
    /// What is typed in each answer box of the current problem, in box order.
    pub inputs: Vec<String>,
    verdicts: Option<Vec<Verdict>>,
    steps_shown: usize,
    /// Problems answered correctly, each counted once.
    correct: usize,
    /// Problems checked at least once.
    attempted: usize,
    counted: bool,
    scored: bool,
}

impl Practice {
    /// Starts at the skill's first problem with empty boxes.
    pub fn new(skill: &Skill) -> Self {
        let mut practice = Self {
            skill: skill.id.clone(),
            index: 0,
            inputs: Vec::new(),
            verdicts: None,
            steps_shown: 0,
            correct: 0,
            attempted: 0,
            counted: false,
            scored: false,
        };
        practice.reset_boxes(skill);
        practice
    }

    pub fn skill_id(&self) -> &str {
        &self.skill
    }

    /// Check that a saved session can belong to this skill before installing it.
    pub fn matches_skill(&self, skill: &Skill) -> bool {
        self.skill == skill.id
            && self.correct <= self.attempted
            && self.problem(skill).is_some_and(|problem| {
                self.inputs.len() == problem.answers.len()
                    && self.inputs.iter().all(|answer| answer.chars().count() <= 4096)
                    && self.verdicts.as_ref().is_none_or(|values| values.len() == problem.answers.len())
                    && self.steps_shown <= problem.steps.len()
                    && self.attempted <= 100_000_000
            })
    }

    /// The problem on screen, or `None` for a skill with no problems.
    pub fn problem<'a>(&self, skill: &'a Skill) -> Option<&'a Problem> {
        skill.problems.get(self.index)
    }

    /// The problem's position, counting from 1, and how many there are.
    pub fn position(&self, skill: &Skill) -> (usize, usize) {
        (self.index + 1, skill.problems.len())
    }

    /// The result of the last check, one per box, until something is typed again.
    pub fn verdicts(&self) -> Option<&[Verdict]> {
        self.verdicts.as_deref()
    }

    /// How many of the problem's worked steps are showing.
    pub fn steps_shown(&self) -> usize {
        self.steps_shown
    }

    /// (correct, attempted) over the whole session.
    pub fn score(&self) -> (usize, usize) {
        (self.correct, self.attempted)
    }

    /// Whether every box of the current problem was right at the last check.
    pub fn solved(&self) -> bool {
        self.verdicts.as_ref().is_some_and(|v| !v.is_empty() && v.iter().all(|x| *x == Verdict::Correct))
    }

    /// Checks every box. The first check of a problem counts it as attempted; the first fully
    /// correct check, made without having shown the worked steps, counts it as correct.
    pub fn check(&mut self, skill: &Skill) {
        let Some(problem) = self.problem(skill) else { return };
        let verdicts: Vec<Verdict> = problem.answers.iter().zip(&self.inputs).map(|(spec, typed)| check(spec, typed)).collect();
        if verdicts.iter().all(|v| *v == Verdict::Empty) {
            self.verdicts = Some(verdicts);
            return;
        }
        if !self.counted {
            self.counted = true;
            self.attempted += 1;
        }
        let all_right = verdicts.iter().all(|v| *v == Verdict::Correct);
        if all_right && !self.scored && self.steps_shown == 0 {
            self.correct += 1;
        }
        if all_right {
            self.scored = true;
        }
        self.verdicts = Some(verdicts);
    }

    /// Shows the next worked step and writes the values it fills into their boxes. Returns
    /// `false` when every step is already showing.
    pub fn show_step(&mut self, skill: &Skill) -> bool {
        let Some(problem) = self.problem(skill) else { return false };
        let Some(step) = problem.steps.get(self.steps_shown) else { return false };
        for (name, value) in &step.fill {
            if let Some(i) = problem.answers.iter().position(|a| &a.name == name)
                && let Some(slot) = self.inputs.get_mut(i)
            {
                *slot = value.clone();
            }
        }
        self.steps_shown += 1;
        self.verdicts = None;
        true
    }

    /// Records that the learner typed, so an old result no longer stands.
    pub fn typed(&mut self) {
        self.verdicts = None;
    }

    /// Moves to the next problem, back to the first after the last.
    pub fn next(&mut self, skill: &Skill) {
        if skill.problems.is_empty() {
            return;
        }
        self.index = (self.index + 1) % skill.problems.len();
        self.reset_boxes(skill);
    }

    fn reset_boxes(&mut self, skill: &Skill) {
        let boxes = self.problem(skill).map_or(0, |p| p.answers.len());
        self.inputs = vec![String::new(); boxes];
        self.verdicts = None;
        self.steps_shown = 0;
        self.counted = false;
        self.scored = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_success_model::{Label, Step};
    use std::collections::BTreeMap;

    fn spec(answer: &str, tol: f64) -> AnswerSpec {
        AnswerSpec { name: "x".into(), label: "X".into(), answer: answer.into(), tol, prefix: None, suffix: None }
    }

    fn skill() -> Skill {
        let problem = |a: &str| Problem {
            label: Label::Generated,
            prompt: format!("? = {a}"),
            answers: vec![AnswerSpec { name: "x".into(), label: "X".into(), answer: a.into(), tol: 0.0, prefix: None, suffix: None }],
            steps: vec![Step { say: "Here.".into(), fill: BTreeMap::from([("x".to_owned(), a.to_owned())]) }],
        };
        Skill {
            id: "s".into(),
            subject: "math".into(),
            grade: "g1".into(),
            code: "A.1".into(),
            title: "S".into(),
            problems: vec![problem("3"), problem("5")],
        }
    }

    #[test]
    fn numbers_are_read_the_way_people_type_them() {
        assert_eq!(parse_number(" 42 "), Some(42.0));
        assert_eq!(parse_number("1,250"), Some(1250.0));
        assert_eq!(parse_number("$75"), Some(75.0));
        assert_eq!(parse_number("\u{2212}3"), Some(-3.0));
        assert_eq!(parse_number("-3"), Some(-3.0));
        assert_eq!(parse_number("0.95"), Some(0.95));
        assert_eq!(parse_number("1 000"), Some(1000.0));
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("seven"), None);
        assert_eq!(parse_number("inf"), None, "infinity is not an answer anyone typed");
    }

    #[test]
    fn numeric_answers_use_the_tolerance() {
        assert_eq!(check(&spec("0.95", 0.0001), "0.95"), Verdict::Correct);
        assert_eq!(check(&spec("0.95", 0.0001), ".95"), Verdict::Correct);
        assert_eq!(check(&spec("0.95", 0.0001), "0.96"), Verdict::Wrong);
        assert_eq!(check(&spec("13.85", 0.01), "13.846"), Verdict::Correct);
        assert_eq!(check(&spec("42", 0.0), "42.0"), Verdict::Correct);
        assert_eq!(check(&spec("42", 0.0), "43"), Verdict::Wrong);
        assert_eq!(check(&spec("42", 0.0), "forty-two"), Verdict::Wrong);
        assert_eq!(check(&spec("42", 0.0), "   "), Verdict::Empty);
    }

    #[test]
    fn text_answers_ignore_case_and_outer_spaces() {
        assert_eq!(check(&spec("Triangle", 0.0), "  triangle "), Verdict::Correct);
        assert_eq!(check(&spec("Triangle", 0.0), "square"), Verdict::Wrong);
    }

    #[test]
    fn a_right_answer_scores_once() {
        let s = skill();
        let mut p = Practice::new(&s);
        assert_eq!(p.position(&s), (1, 2));
        p.inputs[0] = "4".into();
        p.check(&s);
        assert_eq!(p.verdicts(), Some(&[Verdict::Wrong][..]));
        assert!(!p.solved());
        p.inputs[0] = "3".into();
        p.typed();
        assert_eq!(p.verdicts(), None, "typing clears the old result");
        p.check(&s);
        p.check(&s);
        assert!(p.solved());
        assert_eq!(p.score(), (1, 1), "one problem attempted, one correct, however often it is checked");
    }

    #[test]
    fn showing_a_step_fills_the_box_and_forfeits_the_point() {
        let s = skill();
        let mut p = Practice::new(&s);
        assert!(p.show_step(&s));
        assert_eq!(p.inputs[0], "3");
        assert_eq!(p.steps_shown(), 1);
        assert!(!p.show_step(&s), "there is only one step");
        p.check(&s);
        assert!(p.solved());
        assert_eq!(p.score(), (0, 1));
    }

    #[test]
    fn an_empty_check_does_not_count_as_an_attempt() {
        let s = skill();
        let mut p = Practice::new(&s);
        p.check(&s);
        assert_eq!(p.verdicts(), Some(&[Verdict::Empty][..]));
        assert_eq!(p.score(), (0, 0));
    }

    #[test]
    fn next_moves_on_and_wraps_round() {
        let s = skill();
        let mut p = Practice::new(&s);
        p.inputs[0] = "3".into();
        p.check(&s);
        p.next(&s);
        assert_eq!(p.position(&s), (2, 2));
        assert_eq!(p.inputs, vec![String::new()], "the boxes start empty");
        assert_eq!(p.verdicts(), None);
        p.next(&s);
        assert_eq!(p.position(&s), (1, 2));
        assert_eq!(p.score(), (1, 1));
    }

    /// Every worked step in the built-in catalog fills its box with a value the checker
    /// accepts, so following the steps always ends in "Correct".
    #[test]
    fn the_builtin_steps_lead_to_correct_answers() {
        let catalog = match i_success_content::builtin() {
            Ok(c) => c,
            Err(e) => panic!("{e}"),
        };
        let mut problems = 0;
        for skill in &catalog.skills {
            let mut p = Practice::new(skill);
            for _ in 0..skill.problems.len() {
                while p.show_step(skill) {}
                p.check(skill);
                assert!(p.solved(), "{} problem {}: the steps do not reach the answer ({:?})", skill.id, p.position(skill).0, p.inputs);
                problems += 1;
                p.next(skill);
            }
        }
        assert!(problems > 20, "the catalog has its problems ({problems})");
    }
}
