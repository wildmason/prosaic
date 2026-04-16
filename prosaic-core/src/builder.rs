use crate::engine::Engine;
use crate::error::ProsaicError;
use crate::language::{Conjunction, Person, Tense, VerbForm, Voice};

/// A subject in a sentence, optionally with an entity type prefix.
#[derive(Debug, Clone)]
pub struct Subject {
    entity_type: Option<String>,
    name: String,
}

/// Create a subject with an entity type prefix.
/// e.g., `subject("class", "Foo")` renders as "the class Foo".
pub fn subject(entity_type: &str, name: &str) -> Subject {
    Subject {
        entity_type: Some(entity_type.to_string()),
        name: name.to_string(),
    }
}

/// Create a plain subject without an entity type.
pub fn named(name: &str) -> Subject {
    Subject {
        entity_type: None,
        name: name.to_string(),
    }
}

/// A subordinate clause attached to a sentence.
#[derive(Debug, Clone)]
pub struct Clause {
    intro: String,
    amount: Option<usize>,
    noun: Option<String>,
    items: Vec<String>,
    truncate_at: Option<usize>,
    conjunction: Conjunction,
}

impl Clause {
    /// Create a clause introduced by "which {verb}" (e.g., "which impacts").
    pub fn which(verb: &str) -> Self {
        Self {
            intro: format!("which {verb}"),
            amount: None,
            noun: None,
            items: Vec::new(),
            truncate_at: None,
            conjunction: Conjunction::And,
        }
    }

    /// Create a clause with a custom introduction.
    pub fn with_intro(intro: &str) -> Self {
        Self {
            intro: intro.to_string(),
            amount: None,
            noun: None,
            items: Vec::new(),
            truncate_at: None,
            conjunction: Conjunction::And,
        }
    }

    /// Set the count for this clause (e.g., "6 direct consumers").
    pub fn amount(mut self, n: usize) -> Self {
        self.amount = Some(n);
        self
    }

    /// Set the noun that gets pluralized based on the amount.
    pub fn noun(mut self, noun: &str) -> Self {
        self.noun = Some(noun.to_string());
        self
    }

    /// Set the list of items to enumerate.
    pub fn list(mut self, items: &[&str]) -> Self {
        self.items = items.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Truncate the list to show at most `n` items, with "and N more" suffix.
    pub fn truncate(mut self, n: usize) -> Self {
        self.truncate_at = Some(n);
        self
    }

    /// Set the conjunction for joining list items (default: And).
    pub fn conjunction(mut self, conjunction: Conjunction) -> Self {
        self.conjunction = conjunction;
        self
    }

    fn render(&self, engine: &Engine) -> Result<String, ProsaicError> {
        let lang = engine.language();
        let mut parts: Vec<String> = Vec::new();

        if !self.intro.is_empty() {
            parts.push(self.intro.clone());
        }

        if let Some(amount) = self.amount {
            parts.push(amount.to_string());

            if let Some(ref noun) = self.noun {
                parts.push(lang.pluralize(noun, amount));
            }
        } else if let Some(ref noun) = self.noun {
            parts.push(noun.clone());
        }

        if !self.items.is_empty() {
            let display_items = self.truncated_items();
            let refs: Vec<&str> = display_items.iter().map(|s| s.as_str()).collect();
            let joined = lang.join_list(&refs, self.conjunction);
            parts.push(format!("[{joined}]"));
        }

        Ok(parts.join(" "))
    }

    fn truncated_items(&self) -> Vec<String> {
        match self.truncate_at {
            Some(max) if self.items.len() > max => {
                let remaining = self.items.len() - max;
                let mut result: Vec<String> = self.items[..max].to_vec();
                result.push(format!("{remaining} more"));
                result
            }
            _ => self.items.clone(),
        }
    }
}

/// Builder for constructing sentences programmatically.
///
/// The verb is specified either by a simple tense (via `.verb(word, Tense)`)
/// or a full verb form (via `.form(VerbForm)`). The latter unlocks richer
/// constructions like "has been renamed" (present perfect passive) or
/// "is being renamed" (present progressive passive).
#[derive(Debug, Clone)]
pub struct Sentence {
    subject: Option<Subject>,
    verb: Option<String>,
    form: VerbForm,
    voice: Voice,
    person: Person,
    preposition: Option<String>,
    object: Option<String>,
    clauses: Vec<Clause>,
}

impl Sentence {
    pub fn new() -> Self {
        Self {
            subject: None,
            verb: None,
            form: VerbForm::SimplePast,
            voice: Voice::Passive,
            person: Person::Third,
            preposition: None,
            object: None,
            clauses: Vec::new(),
        }
    }

    /// Set the subject of the sentence.
    pub fn subject(mut self, subject: Subject) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Set the verb and its simple tense (Past / Present / Future).
    /// Implies `Aspect::Simple` and `Mood::Indicative`. For richer forms
    /// use [`Sentence::form`].
    pub fn verb(mut self, verb: &str, tense: Tense) -> Self {
        self.verb = Some(verb.to_string());
        self.form = VerbForm::from(tense);
        self
    }

    /// Set the verb form (tense × aspect × mood) directly. Pair with
    /// a verb set via [`Sentence::verb_word`] or a prior [`Sentence::verb`].
    pub fn form(mut self, form: VerbForm) -> Self {
        self.form = form;
        self
    }

    /// Set just the verb word, leaving the existing form in place. Useful
    /// when `form(…)` was used and a base verb is set separately.
    pub fn verb_word(mut self, verb: &str) -> Self {
        self.verb = Some(verb.to_string());
        self
    }

    /// Set the voice (default: Passive).
    ///
    /// - `Voice::Passive`: "The class Foo was renamed to Foobar"
    /// - `Voice::Active`: "The class Foo renamed Foobar"
    pub fn voice(mut self, voice: Voice) -> Self {
        self.voice = voice;
        self
    }

    /// Set the grammatical person used to conjugate auxiliaries
    /// (default: Third).
    pub fn person(mut self, person: Person) -> Self {
        self.person = person;
        self
    }

    /// Set the preposition connecting the verb to the object (default: "to" for passive).
    ///
    /// e.g., `.preposition("into")` → "was converted into Bar"
    pub fn preposition(mut self, prep: &str) -> Self {
        self.preposition = Some(prep.to_string());
        self
    }

    /// Set the direct object.
    pub fn object(mut self, object: &str) -> Self {
        self.object = Some(object.to_string());
        self
    }

    /// Append a subordinate clause.
    pub fn clause(mut self, clause: Clause) -> Self {
        self.clauses.push(clause);
        self
    }

    /// Render the sentence using the given engine's language.
    pub fn render(&self, engine: &Engine) -> Result<String, ProsaicError> {
        let lang = engine.language();
        let mut parts: Vec<String> = Vec::new();

        // Subject
        if let Some(ref subject) = self.subject {
            match &subject.entity_type {
                Some(et) => parts.push(format!("The {} {}", et, subject.name)),
                None => parts.push(subject.name.clone()),
            }
        }

        // Verb phrase — fully composed through the language's verb_phrase hook.
        if let Some(ref verb) = self.verb {
            let phrase = lang.verb_phrase(verb, self.form, self.voice, self.person);
            parts.push(phrase);
        }

        // Object (with preposition)
        if let Some(ref object) = self.object {
            match &self.preposition {
                Some(prep) => parts.push(format!("{prep} {object}")),
                None => {
                    // Default preposition: "to" for passive voice, none for active
                    if self.voice == Voice::Passive {
                        parts.push(format!("to {object}"));
                    } else {
                        parts.push(object.clone());
                    }
                }
            }
        }

        let mut sentence = parts.join(" ");

        // Clauses
        for clause in &self.clauses {
            let rendered = clause.render(engine)?;
            if !rendered.is_empty() {
                sentence.push(' ');
                sentence.push_str(&rendered);
            }
        }

        Ok(sentence)
    }
}

impl Default for Sentence {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::{Conjunction, Language, Person, Tense};

    struct TestLang;

    impl Language for TestLang {
        fn pluralize(&self, word: &str, count: usize) -> String {
            if count == 1 {
                word.to_string()
            } else {
                format!("{word}s")
            }
        }
        fn singularize(&self, word: &str) -> String {
            word.strip_suffix('s').unwrap_or(word).to_string()
        }
        fn article(&self, word: &str) -> &str {
            if word.starts_with(|c: char| "aeiou".contains(c.to_ascii_lowercase())) {
                "an"
            } else {
                "a"
            }
        }
        fn conjugate(&self, verb: &str, tense: Tense, _person: Person) -> String {
            match (verb, tense) {
                ("be", Tense::Past) => "was".to_string(),
                ("be", Tense::Present) => "is".to_string(),
                ("have", Tense::Present) => "has".to_string(),
                (_, Tense::Past) => format!("{verb}ed"),
                (_, Tense::Present) => verb.to_string(),
                (_, Tense::Future) => format!("will {verb}"),
            }
        }
        fn past_participle(&self, verb: &str) -> String {
            format!("{verb}ed")
        }
        fn present_participle(&self, verb: &str) -> String {
            format!("{verb}ing")
        }
        fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String {
            let conj = match conjunction {
                Conjunction::And => "and",
                Conjunction::Or => "or",
            };
            match items.len() {
                0 => String::new(),
                1 => items[0].to_string(),
                2 => format!("{} {conj} {}", items[0], items[1]),
                _ => {
                    let head = items[..items.len() - 1].join(", ");
                    format!("{head}, {conj} {}", items[items.len() - 1])
                }
            }
        }
        fn ordinal(&self, n: usize) -> String {
            format!("{n}th")
        }
        fn number_to_words(&self, n: usize) -> String {
            format!("<{n}>")
        }
    }

    fn test_engine() -> Engine {
        Engine::new(TestLang)
    }

    #[test]
    fn passive_voice_past_tense() {
        let engine = test_engine();
        let s = Sentence::new()
            .subject(subject("class", "Foo"))
            .verb("rename", Tense::Past)
            .object("Foobar")
            .render(&engine)
            .unwrap();

        assert_eq!(s, "The class Foo was renameed to Foobar");
    }

    #[test]
    fn active_voice_past_tense() {
        let engine = test_engine();
        let s = Sentence::new()
            .subject(subject("class", "Foo"))
            .verb("rename", Tense::Past)
            .object("Foobar")
            .voice(Voice::Active)
            .render(&engine)
            .unwrap();

        assert_eq!(s, "The class Foo renameed Foobar");
    }

    #[test]
    fn passive_voice_with_clause() {
        let engine = test_engine();
        let s = Sentence::new()
            .subject(subject("class", "Foo"))
            .verb("rename", Tense::Past)
            .object("Foobar")
            .clause(
                Clause::which("impacts")
                    .amount(6)
                    .noun("direct consumer"),
            )
            .render(&engine)
            .unwrap();

        assert_eq!(
            s,
            "The class Foo was renameed to Foobar which impacts 6 direct consumers"
        );
    }

    #[test]
    fn passive_voice_with_clause_and_list() {
        let engine = test_engine();
        let s = Sentence::new()
            .subject(subject("class", "Foo"))
            .verb("rename", Tense::Past)
            .object("Foobar")
            .clause(
                Clause::which("impacts")
                    .amount(6)
                    .noun("direct consumer")
                    .list(&["Baz", "Qux", "Quux", "Corge", "Grault", "Garply"])
                    .truncate(3),
            )
            .render(&engine)
            .unwrap();

        assert_eq!(
            s,
            "The class Foo was renameed to Foobar which impacts 6 direct consumers \
             [Baz, Qux, Quux, and 3 more]"
        );
    }

    #[test]
    fn passive_voice_no_object() {
        let engine = test_engine();
        let s = Sentence::new()
            .subject(named("UserService"))
            .verb("modify", Tense::Past)
            .render(&engine)
            .unwrap();

        assert_eq!(s, "UserService was modifyed");
    }

    #[test]
    fn active_voice_no_object() {
        let engine = test_engine();
        let s = Sentence::new()
            .subject(named("UserService"))
            .verb("modify", Tense::Past)
            .voice(Voice::Active)
            .render(&engine)
            .unwrap();

        assert_eq!(s, "UserService modifyed");
    }

    #[test]
    fn custom_preposition() {
        let engine = test_engine();
        let s = Sentence::new()
            .subject(subject("class", "Foo"))
            .verb("convert", Tense::Past)
            .preposition("into")
            .object("Bar")
            .render(&engine)
            .unwrap();

        assert_eq!(s, "The class Foo was converted into Bar");
    }

    #[test]
    fn passive_present_tense() {
        let engine = test_engine();
        let s = Sentence::new()
            .subject(subject("module", "Core"))
            .verb("export", Tense::Present)
            .clause(
                Clause::with_intro("")
                    .amount(5)
                    .noun("component"),
            )
            .render(&engine)
            .unwrap();

        assert_eq!(s, "The module Core is exported 5 components");
    }

    #[test]
    fn passive_future_tense() {
        let engine = test_engine();
        let s = Sentence::new()
            .subject(subject("interface", "Foo"))
            .verb("deprecate", Tense::Future)
            .render(&engine)
            .unwrap();

        assert_eq!(s, "The interface Foo will be deprecateed");
    }

    #[test]
    fn clause_no_truncation_needed() {
        let engine = test_engine();
        let s = Sentence::new()
            .subject(subject("method", "getData"))
            .verb("delete", Tense::Past)
            .clause(
                Clause::which("impacts")
                    .amount(2)
                    .noun("caller")
                    .list(&["ComponentA", "ComponentB"]),
            )
            .render(&engine)
            .unwrap();

        assert_eq!(
            s,
            "The method getData was deleteed which impacts 2 callers [ComponentA and ComponentB]"
        );
    }
}
