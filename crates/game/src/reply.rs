//! Mock AI reply drafting (story 007). `generate_draft` stands in for the
//! backend draft endpoint (see api-changerequests/card-flow.md) with a
//! deterministic template engine over the message's enrichment metadata:
//! empathetic tone by sentiment, a concrete next step by category, and the
//! language of the original message. The steering note from a "recharge"
//! nudges the templates (keywords) and reshuffles phrasing (hash), so a new
//! note reliably produces a new draft. Swapping in the real AI later only
//! touches this module.

use shared::Message;

use crate::meta::{Category, MessageMeta, Sentiment};

/// Language the reply is written in, matched to the original message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    English,
    Dutch,
}

/// Dutch function words that almost never appear as standalone English words.
const DUTCH_HINTS: &[&str] = &[
    "de", "het", "een", "niet", "ik", "wij", "jullie", "mijn", "uw", "dit", "dat", "voor",
    "naar", "graag", "bedankt", "alvast", "kunnen", "kunt", "wordt", "werkt", "mij", "ons",
    "geen", "maar", "ook", "bij", "wel", "even",
];

/// Guess the language of a message from function-word counts. Deterministic
/// mock of the model's language detection; three hits is enough signal to
/// avoid loanword false positives in English text.
pub fn detect_language(text: &str) -> Lang {
    let hits = text
        .to_lowercase()
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| DUTCH_HINTS.contains(w))
        .count();
    if hits >= 3 {
        Lang::Dutch
    } else {
        Lang::English
    }
}

/// What the steering note asks for, mined from keywords (EN + NL).
struct Steering {
    shorter: bool,
    formal: bool,
    refund: bool,
    apology: bool,
}

fn steering(note: &str) -> Steering {
    let n = note.to_lowercase();
    let has = |keys: &[&str]| keys.iter().any(|k| n.contains(k));
    Steering {
        shorter: has(&["short", "kort", "brief", "compact"]),
        formal: has(&["formal", "formeel", "zakelijk"]),
        refund: has(&["refund", "terugbetal", "geld terug"]),
        apology: has(&["apolog", "sorry", "excus"]),
    }
}

/// FNV-1a, the deterministic seed for phrasing choices.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

/// Pick from `options` using byte `slot` of the seed plus the recharge count,
/// so every recharge visibly changes the phrasing even with the same note.
fn pick<'a>(options: &[&'a str], seed: u64, slot: u32, variant: u32) -> &'a str {
    let idx = ((seed >> (slot * 8)) as usize).wrapping_add(variant as usize);
    options[idx % options.len()]
}

/// Draft a reply to `message`. Deterministic: the same message, steering
/// note, and recharge count always produce the same draft.
///
/// * `meta` - enrichment for the message (drives tone and next step).
/// * `note` - the player's steering note; empty means none.
/// * `variant` - recharge count, 0 for the first draft.
pub fn generate_draft(message: &Message, meta: &MessageMeta, note: &str, variant: u32) -> String {
    let lang = detect_language(&format!("{} {}", message.subject, message.body));
    let steer = steering(note);
    let mut seed_input = message.body.clone().into_bytes();
    seed_input.extend_from_slice(note.as_bytes());
    let seed = fnv1a(&seed_input);
    let subject = message.subject.trim();

    let greeting = match (lang, steer.formal) {
        (Lang::English, false) => "Hi there,",
        (Lang::English, true) => "Dear customer,",
        (Lang::Dutch, false) => "Beste klant,",
        (Lang::Dutch, true) => "Geachte klant,",
    };
    let signoff = match (lang, steer.formal) {
        (Lang::English, false) => "Best,",
        (Lang::English, true) => "Kind regards,",
        (Lang::Dutch, _) => "Met vriendelijke groet,",
    };

    if steer.shorter {
        let action = short_action(meta.category, lang);
        let echo = match lang {
            Lang::English => format!("Thanks for reaching out about \"{subject}\" - {action}"),
            Lang::Dutch => format!("Bedankt voor uw bericht over \"{subject}\" - {action}"),
        };
        return format!("{greeting}\n\n{echo}\n\n{signoff}\nMeridian Support");
    }

    let mut empathy = empathy_line(meta.sentiment, subject, lang, seed, variant);
    if steer.apology {
        let right = match lang {
            Lang::English => "You are completely right to raise this. ",
            Lang::Dutch => "U heeft helemaal gelijk dat u dit aankaart. ",
        };
        empathy = format!("{right}{empathy}");
    }

    let mut action = action_line(meta.category, lang).to_string();
    if steer.refund {
        action.push_str(match lang {
            Lang::English => {
                " Of course we'll refund any amount charged in error - no further action is needed on your side."
            }
            Lang::Dutch => {
                " Uiteraard storten we onterecht afgeschreven bedragen terug - u hoeft verder niets te doen."
            }
        });
    }

    let closing = match lang {
        Lang::English => pick(
            &[
                "Is there anything else I can help you with?",
                "Please don't hesitate to reach out if anything else comes up.",
            ],
            seed,
            1,
            variant,
        ),
        Lang::Dutch => pick(
            &[
                "Kan ik verder nog iets voor u betekenen?",
                "Neem gerust weer contact op als er nog iets speelt.",
            ],
            seed,
            1,
            variant,
        ),
    };

    format!("{greeting}\n\n{empathy}\n{action}\n\n{closing}\n\n{signoff}\nMeridian Support")
}

fn empathy_line(sentiment: Sentiment, subject: &str, lang: Lang, seed: u64, variant: u32) -> String {
    let options: &[&str] = match (lang, sentiment) {
        (Lang::English, Sentiment::Negative | Sentiment::Angry) => &[
            "I'm sorry about the trouble with \"{s}\" - that's not the experience we want you to have.",
            "Thank you for flagging \"{s}\", and I'm sorry for the hassle it has caused.",
        ],
        (Lang::English, Sentiment::Positive) => &[
            "Thank you for the kind words about \"{s}\" - that genuinely made our day.",
            "We really appreciate you sharing \"{s}\" with us.",
        ],
        (Lang::English, Sentiment::Neutral) => &[
            "Thanks for reaching out about \"{s}\".",
            "Thank you for your message about \"{s}\".",
        ],
        (Lang::Dutch, Sentiment::Negative | Sentiment::Angry) => &[
            "Wat vervelend om te lezen over \"{s}\" - dat is niet de ervaring die we u willen geven.",
            "Bedankt voor het melden van \"{s}\", en excuses voor het ongemak.",
        ],
        (Lang::Dutch, Sentiment::Positive) => &[
            "Dank voor de mooie woorden over \"{s}\" - daar worden we blij van.",
            "Fijn dat u \"{s}\" met ons deelt.",
        ],
        (Lang::Dutch, Sentiment::Neutral) => &[
            "Bedankt voor uw bericht over \"{s}\".",
            "Dank voor uw bericht over \"{s}\".",
        ],
    };
    pick(options, seed, 0, variant).replace("{s}", subject)
}

fn action_line(category: Category, lang: Lang) -> &'static str {
    match (lang, category) {
        (Lang::English, Category::Billing) => {
            "I've flagged this with our billing team; you'll see the correction reflected on your account within two business days."
        }
        (Lang::English, Category::Complaint) => {
            "I've escalated this to the responsible team and we will make it right - expect a concrete follow-up from us shortly."
        }
        (Lang::English, Category::Question) => {
            "Here's what I can tell you right away, and I'm happy to dig deeper if anything is still unclear."
        }
        (Lang::English, Category::Feedback) => {
            "I've shared your note with the team - feedback like this directly shapes what we improve next."
        }
        (Lang::Dutch, Category::Billing) => {
            "Ik heb dit doorgezet naar ons facturatieteam; u ziet de correctie binnen twee werkdagen terug op uw account."
        }
        (Lang::Dutch, Category::Complaint) => {
            "Ik heb dit intern doorgezet naar het juiste team en we gaan dit rechtzetten - u hoort snel van ons met een concrete oplossing."
        }
        (Lang::Dutch, Category::Question) => {
            "Dit kan ik u alvast vertellen, en ik duik er graag verder in als iets onduidelijk blijft."
        }
        (Lang::Dutch, Category::Feedback) => {
            "Ik heb uw bericht gedeeld met het team - dit soort feedback bepaalt wat we als eerste verbeteren."
        }
    }
}

fn short_action(category: Category, lang: Lang) -> &'static str {
    match (lang, category) {
        (Lang::English, Category::Billing) => "our billing team will correct this within two business days.",
        (Lang::English, Category::Complaint) => "we're on it and will make it right.",
        (Lang::English, Category::Question) => "happy to help; the short answer is on its way.",
        (Lang::English, Category::Feedback) => "thank you, this went straight to the team.",
        (Lang::Dutch, Category::Billing) => "ons facturatieteam corrigeert dit binnen twee werkdagen.",
        (Lang::Dutch, Category::Complaint) => "we pakken dit direct op en zetten het recht.",
        (Lang::Dutch, Category::Question) => "we helpen u graag; het korte antwoord volgt hieronder.",
        (Lang::Dutch, Category::Feedback) => "dank u, dit is direct doorgegeven aan het team.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta;
    use shared::{Channel, MessageStatus};

    fn msg(subject: &str, body: &str) -> Message {
        Message {
            id: 7,
            channel: Channel::Email,
            sender: "test@example.com".into(),
            subject: subject.into(),
            body: body.into(),
            received_at: 0,
            status: MessageStatus::Open,
        }
    }

    fn draft(m: &Message, note: &str, variant: u32) -> String {
        generate_draft(m, &meta::enrich(m), note, variant)
    }

    #[test]
    fn drafts_are_deterministic() {
        let m = msg("Invoice #4821 charged twice", "I was billed twice. Please refund one charge.");
        assert_eq!(draft(&m, "", 0), draft(&m, "", 0));
        assert_eq!(draft(&m, "offer refund", 2), draft(&m, "offer refund", 2));
    }

    #[test]
    fn complaint_reply_is_empathetic_and_addresses_the_message() {
        let m = msg(
            "Slow delivery",
            "It took three weeks to arrive and nobody answered my emails in between.",
        );
        let d = draft(&m, "", 0);
        assert!(d.contains("sorry") || d.contains("vervelend"), "no empathy in: {d}");
        assert!(d.contains("Slow delivery"), "does not echo the message: {d}");
        assert!(d.contains("Meridian Support"));
    }

    #[test]
    fn dutch_message_gets_a_dutch_reply() {
        let m = msg(
            "Wachtwoord reset werkt niet",
            "De reset link werkt niet en ik kan niet meer inloggen. Kunnen jullie mij helpen? Alvast bedankt.",
        );
        let d = draft(&m, "", 0);
        assert!(d.starts_with("Beste klant,"), "not Dutch: {d}");
        assert!(d.contains("Met vriendelijke groet,"));
    }

    #[test]
    fn english_message_gets_an_english_reply() {
        let m = msg("Can't reset my password", "The reset link keeps saying token expired.");
        assert!(draft(&m, "", 0).starts_with("Hi there,"));
    }

    #[test]
    fn recharge_without_a_note_still_changes_the_draft() {
        let m = msg("Invoice #4821 charged twice", "I was billed twice for my March invoice.");
        assert_ne!(draft(&m, "", 0), draft(&m, "", 1));
    }

    #[test]
    fn steering_note_keywords_shape_the_draft() {
        let m = msg("Invoice #4821 charged twice", "I was billed twice for my March invoice.");
        let plain = draft(&m, "", 0);
        let short = draft(&m, "keep it short", 1);
        let formal = draft(&m, "more formal", 1);
        let refund = draft(&m, "offer a refund", 1);
        assert!(short.lines().count() < plain.lines().count(), "not shorter: {short}");
        assert!(formal.starts_with("Dear customer,"), "not formal: {formal}");
        assert!(refund.contains("refund"), "no refund offer: {refund}");
        assert_ne!(plain, draft(&m, "try a different angle", 1));
    }
}
