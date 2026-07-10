//! Mock inbox and demo course generator (story 002). Real course generation
//! is an AI backend job that does not exist yet (see
//! api-changerequests/course-search.md); `generate_course` stands in with a
//! seeded template+variation generator, so a full 50-hurdle demo run plays
//! without any real customer data. The rest of the game only sees `Message`
//! values, so swapping in the backend stays local to this module.
//!
//! Controls: `ZD_SEED=<u64>` picks the seed, `ZD_COURSE=chill|normal|nightmare`
//! the difficulty preset, `ZD_COUNT=<n>` the course length. In-game, R relays
//! a fresh course (the story's reset flag) and D cycles the preset.
//!
//! Designed together with hazards.rs: templates are themed and each theme
//! emits the keywords the hazard clusterer matches on, so spikes (e.g. a
//! burst of checkout failures on a nightmare Monday) surface as zones.

use shared::{Channel, Message, MessageStatus};

/// Default number of messages in a generated course (story 002: at least 50).
pub const COURSE_LEN: usize = 50;

/// Default demo seed, chosen so every preset shows the full variety
/// (all channels, praise, Dutch messages, and a nightmare checkout spike).
pub const DEFAULT_SEED: u64 = 11;

const BASE_TS: i64 = 1_780_000_000;

// ---- seeded RNG ----

/// Tiny xorshift64* RNG: deterministic per seed, no dependencies.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // One splitmix64 step spreads the seed so nearby seeds diverge fast;
        // xorshift state must be non-zero.
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        Self(z.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }

    fn chance(&mut self, p: f32) -> bool {
        ((self.next() % 10_000) as f32) < p * 10_000.0
    }

    fn pick<'a, T: ?Sized>(&mut self, items: &'a [&'a T]) -> &'a T {
        items[self.below(items.len())]
    }
}

// ---- difficulty presets ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    ChillJog,
    NormalShift,
    NightmareMonday,
}

impl Difficulty {
    pub fn label(self) -> &'static str {
        match self {
            Difficulty::ChillJog => "chill jog",
            Difficulty::NormalShift => "normal shift",
            Difficulty::NightmareMonday => "nightmare monday",
        }
    }

    pub fn next(self) -> Difficulty {
        match self {
            Difficulty::ChillJog => Difficulty::NormalShift,
            Difficulty::NormalShift => Difficulty::NightmareMonday,
            Difficulty::NightmareMonday => Difficulty::ChillJog,
        }
    }

    fn parse(s: &str) -> Option<Difficulty> {
        let s = s.to_lowercase();
        if s.contains("chill") || s.contains("jog") {
            Some(Difficulty::ChillJog)
        } else if s.contains("night") || s.contains("monday") {
            Some(Difficulty::NightmareMonday)
        } else if s.contains("normal") || s.contains("shift") {
            Some(Difficulty::NormalShift)
        } else {
            None
        }
    }

    /// Relative odds per theme; order matches `Theme::ALL`.
    fn theme_weights(self) -> [u32; 8] {
        match self {
            //                       chk dbl pwd del api pln pra adm
            Difficulty::ChillJog => [1, 2, 2, 2, 1, 4, 5, 3],
            Difficulty::NormalShift => [2, 3, 3, 3, 2, 3, 2, 2],
            Difficulty::NightmareMonday => [3, 4, 3, 4, 4, 1, 1, 1],
        }
    }

    /// Length of the contiguous checkout-failure burst (story 010's spike).
    fn spike_len(self) -> usize {
        match self {
            Difficulty::ChillJog => 0,
            Difficulty::NormalShift => 6,
            Difficulty::NightmareMonday => 14,
        }
    }

    /// Chance a message gets time-pressure wording ("URGENT", "ASAP").
    fn urgent_p(self) -> f32 {
        match self {
            Difficulty::ChillJog => 0.04,
            Difficulty::NormalShift => 0.18,
            Difficulty::NightmareMonday => 0.40,
        }
    }

    /// Chance a message gets angry wording ("unacceptable!!!").
    fn angry_p(self) -> f32 {
        match self {
            Difficulty::ChillJog => 0.04,
            Difficulty::NormalShift => 0.15,
            Difficulty::NightmareMonday => 0.40,
        }
    }
}

/// Which course to generate: seed + difficulty + length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CourseSpec {
    pub seed: u64,
    pub difficulty: Difficulty,
    pub count: usize,
}

impl CourseSpec {
    pub fn from_env() -> Self {
        let seed = std::env::var("ZD_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_SEED);
        let difficulty = std::env::var("ZD_COURSE")
            .ok()
            .and_then(|s| Difficulty::parse(&s))
            .unwrap_or(Difficulty::NormalShift);
        let count = std::env::var("ZD_COUNT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(COURSE_LEN);
        Self {
            seed,
            difficulty,
            count,
        }
    }

    /// The story's reset flag: derive the spec for a freshly relaid course.
    pub fn reseeded(self) -> Self {
        Self {
            seed: self
                .seed
                .wrapping_mul(0x5851_F42D_4C95_7F2D)
                .wrapping_add(0x1405_7B7E_F767_814F),
            ..self
        }
    }
}

// ---- themes and templates ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Theme {
    CheckoutFailure,
    DoubleCharge,
    PasswordReset,
    DeliveryDelay,
    ApiErrors,
    PlanQuestion,
    Praise,
    AccountAdmin,
}

impl Theme {
    const ALL: [Theme; 8] = [
        Theme::CheckoutFailure,
        Theme::DoubleCharge,
        Theme::PasswordReset,
        Theme::DeliveryDelay,
        Theme::ApiErrors,
        Theme::PlanQuestion,
        Theme::Praise,
        Theme::AccountAdmin,
    ];

    /// May receive time-pressure wording.
    fn escalates_urgent(self) -> bool {
        self != Theme::Praise
    }

    /// May receive angry wording.
    fn escalates_angry(self) -> bool {
        !matches!(self, Theme::Praise | Theme::PlanQuestion)
    }
}

const FIRST_NAMES: &[&str] = &[
    "Sanne", "Daan", "Femke", "Ruben", "Lotte", "Bram", "Anouk", "Jesse", "Nora", "Tim", "Eva",
    "Sem",
];
const LAST_NAMES: &[&str] = &[
    "de Vries", "Jansen", "Bakker", "Visser", "Smit", "Meijer", "Mulder", "de Boer", "Peters",
    "Hendriks",
];
const ERROR_CODES: &[&str] = &["CHK-500", "PAY-502", "ERR-500", "PAY-419"];
const DAYS_EN: &[&str] = &["Monday", "Tuesday", "yesterday", "last Friday"];
const DAYS_NL: &[&str] = &["maandag", "gisteren", "vrijdag", "dinsdag"];
const STREETS_NL: &[&str] = &[
    "Keizersgracht",
    "Stationsweg",
    "Dorpsstraat",
    "Hoofdstraat",
    "Marktplein",
];
const CITIES_NL: &[&str] = &["Amsterdam", "Utrecht", "Groningen", "Eindhoven", "Leiden"];

struct Draft {
    channel: Channel,
    subject: String,
    body: String,
    dutch: bool,
}

fn sender_for(rng: &mut Rng, channel: Channel) -> String {
    match channel {
        Channel::Email => {
            let first = rng.pick(FIRST_NAMES);
            let last = rng.pick(LAST_NAMES);
            format!(
                "{}.{}@example.com",
                first[..1].to_lowercase(),
                last.to_lowercase().replace(' ', "")
            )
        }
        Channel::WebForm => "Contact form".to_string(),
        Channel::Review => rng
            .pick(&[
                "TrustSpot review",
                "AppStore review",
                "PlayStore review",
                "Webshop review",
            ])
            .to_string(),
        Channel::Ticket => format!("Ticket #{}", rng.range(7100, 9899)),
    }
}

/// Compose one message for a theme. Variants mix channels, tones, and
/// languages (some Dutch); escalation flags bolt on urgency/anger wording the
/// AI enrichment (meta.rs) recognizes.
fn compose(theme: Theme, rng: &mut Rng) -> Draft {
    let ord = rng.range(41_000, 49_999);
    let inv = rng.range(4_000, 4_999);
    let err = rng.pick(ERROR_CODES);
    let day = rng.pick(DAYS_EN);
    let day_nl = rng.pick(DAYS_NL);
    let weeks = rng.range(2, 5);
    let seats = rng.range(3, 8);
    let amount = rng.range(19, 249);
    let street = rng.pick(STREETS_NL);
    let number = rng.range(1, 180);
    let city = rng.pick(CITIES_NL);

    let (channel, subject, body, dutch) = match theme {
        Theme::CheckoutFailure => match rng.below(4) {
            0 => (
                Channel::WebForm,
                "Checkout fails at the payment step".to_string(),
                format!(
                    "My cart is fine but the payment step at checkout fails every time with error {err}. \
                     I have tried two different cards for order #{ord}."
                ),
                false,
            ),
            1 => (
                Channel::Email,
                format!("Card declined at checkout, order #{ord}"),
                format!(
                    "Trying to pay for order #{ord} but my card is declined at checkout every time, \
                     and the retry gives error {err}. My bank says nothing is wrong on their side."
                ),
                false,
            ),
            2 => (
                Channel::WebForm,
                "Afrekenen lukt niet, betaling blijft hangen".to_string(),
                format!(
                    "Bij het afrekenen loopt de betaalstap steeds vast met foutcode {err}. \
                     Bestelling #{ord} staat nog in mijn winkelwagen. Kunnen jullie hiernaar kijken?"
                ),
                true,
            ),
            _ => (
                Channel::Ticket,
                "Payment fails at checkout for all our staff accounts".to_string(),
                format!(
                    "Since {day} every checkout attempt in our team account fails at the payment \
                     step with {err}. This blocks our sales."
                ),
                false,
            ),
        },
        Theme::DoubleCharge => match rng.below(3) {
            0 => (
                Channel::Email,
                format!("Invoice #{inv} charged twice"),
                format!(
                    "I was billed twice for invoice #{inv} this month. Please refund one of the \
                     charges. Order reference #{ord}."
                ),
                false,
            ),
            1 => (
                Channel::WebForm,
                "Refund for duplicate charge".to_string(),
                format!(
                    "There is a duplicate charge of EUR {amount} on my statement for the \
                     subscription renewal. Can you refund the second charge?"
                ),
                false,
            ),
            _ => (
                Channel::Email,
                format!("Dubbel afgeschreven voor factuur #{inv}"),
                format!(
                    "Er is deze maand twee keer afgeschreven voor factuur #{inv}. \
                     Graag een van de twee bedragen terugstorten."
                ),
                true,
            ),
        },
        Theme::PasswordReset => match rng.below(4) {
            0 => (
                Channel::WebForm,
                "Can't reset my password".to_string(),
                "The reset link in the email says 'token expired' even when I click it right \
                 away. Can't log in to my account."
                    .to_string(),
                false,
            ),
            1 => (
                Channel::Ticket,
                "Whole team locked out after SSO change".to_string(),
                "Since the SSO change this morning nobody can log in and password resets \
                 bounce. We are locked out of the dashboard."
                    .to_string(),
                false,
            ),
            2 => (
                Channel::WebForm,
                "Wachtwoord resetten lukt niet".to_string(),
                "De resetlink geeft steeds 'token verlopen', ook als ik er direct op klik. \
                 Ik kan niet meer inloggen op mijn account."
                    .to_string(),
                true,
            ),
            _ => (
                Channel::Email,
                "Password reset mail never arrives".to_string(),
                "I requested a password reset three times but the mail never arrives. \
                 Already checked spam. Can you trigger it manually?"
                    .to_string(),
                false,
            ),
        },
        Theme::DeliveryDelay => match rng.below(4) {
            0 => (
                Channel::Email,
                format!("Order #{ord} still not delivered"),
                format!(
                    "My order #{ord} was shipped {weeks} weeks ago and still has not arrived. \
                     The tracking page has not updated since {day}."
                ),
                false,
            ),
            1 => (
                Channel::Review,
                "2/5 - Slow delivery, nice product".to_string(),
                format!(
                    "The product itself is nice, but delivery took {weeks} weeks and nobody \
                     answered my emails in between."
                ),
                false,
            ),
            2 => (
                Channel::Email,
                format!("Bestelling #{ord} nog steeds niet geleverd"),
                format!(
                    "Mijn pakket is al {weeks} weken onderweg en de track and trace wordt niet \
                     meer bijgewerkt. Wanneer wordt bestelling #{ord} bezorgd?"
                ),
                true,
            ),
            _ => (
                Channel::WebForm,
                "Where is my package?".to_string(),
                format!(
                    "The delivery estimate for order #{ord} was {day} and the package still \
                     has not arrived. Where is it?"
                ),
                false,
            ),
        },
        Theme::ApiErrors => match rng.below(4) {
            0 => (
                Channel::Ticket,
                "API returns 500 on bulk export".to_string(),
                format!(
                    "Since {day} our nightly export job fails with a 500. Nothing changed on \
                     our side. This blocks our reporting."
                ),
                false,
            ),
            1 => (
                Channel::Ticket,
                "Webhook retries flooding our endpoint".to_string(),
                "Your webhook retry policy is hammering our server 50 times a minute for one \
                 failed delivery. Can you cap it?"
                    .to_string(),
                false,
            ),
            2 => (
                Channel::Ticket,
                "Export API intermittently returns 500 errors".to_string(),
                format!(
                    "Roughly one in five calls to the export API returns a 500 since {day}. \
                     Retries eventually succeed but our sync is slow."
                ),
                false,
            ),
            _ => (
                Channel::Ticket,
                "API geeft 500 bij bulk-export".to_string(),
                format!(
                    "Sinds {day_nl} geeft de export-API een 500 bij onze nachtelijke \
                     synchronisatie. Aan onze kant is niets veranderd."
                ),
                true,
            ),
        },
        Theme::PlanQuestion => match rng.below(4) {
            0 => (
                Channel::WebForm,
                "Question about the family plan".to_string(),
                format!(
                    "Do you offer a family plan? We would need {seats} seats. I could not find \
                     it on the pricing page."
                ),
                false,
            ),
            1 => (
                Channel::Email,
                "How do I add more seats?".to_string(),
                format!(
                    "We are hiring and need {seats} extra seats next month. How do I add them \
                     to our current plan, and what does it cost?"
                ),
                false,
            ),
            2 => (
                Channel::WebForm,
                "Vraag over een gezinsabonnement".to_string(),
                format!(
                    "Bieden jullie een gezinsabonnement aan? We zouden {seats} accounts nodig \
                     hebben. Ik kon het niet vinden op de prijzenpagina."
                ),
                true,
            ),
            _ => (
                Channel::Email,
                "Annual billing available?".to_string(),
                format!(
                    "Is there a discount if we switch to annual billing? We are currently on \
                     the monthly plan with {seats} seats."
                ),
                false,
            ),
        },
        Theme::Praise => match rng.below(4) {
            0 => (
                Channel::Review,
                "5/5 - Support turned it around".to_string(),
                "Had an issue with my order but support fixed it within a day. Great service, \
                 thanks!"
                    .to_string(),
                false,
            ),
            1 => (
                Channel::Review,
                "5/5 - Fast and friendly help".to_string(),
                "Quick reply, clear answer, problem solved. Great experience, thanks a lot!"
                    .to_string(),
                false,
            ),
            2 => (
                Channel::Review,
                "5/5 - Top geholpen".to_string(),
                "Snel en vriendelijk geholpen met mijn vraag. Echt een topservice, bedankt!"
                    .to_string(),
                true,
            ),
            _ => (
                Channel::Email,
                "Thanks for the quick fix".to_string(),
                "Just wanted to say thanks: the sync issue from last week is fixed and \
                 everything runs great now."
                    .to_string(),
                false,
            ),
        },
        Theme::AccountAdmin => match rng.below(4) {
            0 => (
                Channel::Email,
                "Change of address before next shipment".to_string(),
                format!(
                    "We are moving offices on the 1st. Please send the next shipment to \
                     {street} {number}, {city}."
                ),
                false,
            ),
            1 => (
                Channel::Email,
                "Cancel my subscription".to_string(),
                "I would like to cancel my subscription effective next month. Nothing wrong \
                 with the service, we simply no longer need it."
                    .to_string(),
                false,
            ),
            2 => (
                Channel::WebForm,
                "Adreswijziging doorgeven".to_string(),
                format!(
                    "Per de eerste van de maand verhuizen wij naar {street} {number} in {city}. \
                     Willen jullie het adres aanpassen voor de volgende levering?"
                ),
                true,
            ),
            _ => (
                Channel::WebForm,
                "Update the billing contact".to_string(),
                "Our finance team changed; please update the billing contact for invoices to \
                 finance@example.com."
                    .to_string(),
                false,
            ),
        },
    };

    Draft {
        channel,
        subject,
        body,
        dutch,
    }
}

fn weighted_theme(rng: &mut Rng, weights: &[u32; 8]) -> Theme {
    let total: u32 = weights.iter().sum();
    let mut roll = (rng.next() % u64::from(total)) as u32;
    for (theme, w) in Theme::ALL.iter().zip(weights) {
        if roll < *w {
            return *theme;
        }
        roll -= w;
    }
    Theme::ALL[0]
}

/// Generate the demo course: deterministic per spec, at least one message.
pub fn generate_course(spec: &CourseSpec) -> Vec<Message> {
    let count = spec.count.max(1);
    let mut rng = Rng::new(spec.seed ^ ((spec.difficulty as u64 + 1) << 56));

    let weights = spec.difficulty.theme_weights();
    let mut themes: Vec<Theme> = (0..count).map(|_| weighted_theme(&mut rng, &weights)).collect();

    // The "many messages about one issue in a short period" spike (story 010):
    // a contiguous burst of checkout failures, sized by the preset.
    let spike_len = spec.difficulty.spike_len().min(count);
    let mut spike = 0..0;
    if spike_len > 0 {
        let lead = (count / 6).min(count - spike_len);
        let slack = count - spike_len - lead;
        let start = lead + if slack > 0 { rng.below(slack + 1) } else { 0 };
        spike = start..start + spike_len;
        for slot in &mut themes[spike.clone()] {
            *slot = Theme::CheckoutFailure;
        }
    }

    let mut ts = BASE_TS;
    themes
        .iter()
        .enumerate()
        .map(|(i, &theme)| {
            // Spike messages arrive in a tight burst; the rest trickle in.
            ts += if spike.contains(&i) {
                rng.range(20, 90)
            } else {
                rng.range(180, 1200)
            };
            let urgent = theme.escalates_urgent() && rng.chance(spec.difficulty.urgent_p());
            let angry = theme.escalates_angry() && rng.chance(spec.difficulty.angry_p());
            let draft = compose(theme, &mut rng);
            let mut subject = draft.subject;
            let mut body = draft.body;
            if urgent {
                subject = format!("URGENT: {subject}");
                body.push_str(if draft.dutch {
                    " Dit moet vandaag opgelost worden, ASAP."
                } else {
                    " We need a solution today, ASAP."
                });
            }
            if angry {
                body.push_str(if draft.dutch {
                    " Dit is echt onacceptabel!!!"
                } else {
                    " This is unacceptable!!!"
                });
            }
            Message {
                id: i as u64 + 1,
                channel: draft.channel,
                sender: sender_for(&mut rng, draft.channel),
                subject,
                body,
                received_at: ts,
                status: MessageStatus::Open,
            }
        })
        .collect()
}

// ---- legacy fixtures (kept for track.rs tests and small demos) ----

const SAMPLES: &[(&str, Channel, &str, &str)] = &[
    (
        "s.devries@example.com",
        Channel::Email,
        "Invoice #4821 charged twice",
        "Hi, I was billed twice for my March invoice. Can you refund one of the charges? Order reference 4821.",
    ),
    (
        "Contact form",
        Channel::WebForm,
        "Can't reset my password",
        "The reset link in the email keeps saying 'token expired' even when I click it right away.",
    ),
    (
        "TrustSpot review",
        Channel::Review,
        "★★☆☆☆ Slow delivery, great product",
        "Product itself is great, but it took three weeks to arrive and nobody answered my emails in between.",
    ),
    (
        "Ticket #7719",
        Channel::Ticket,
        "API returns 500 on bulk export",
        "Since yesterday our nightly export job fails with a 500. Nothing changed on our side. This blocks our reporting.",
    ),
    (
        "m.jansen@example.com",
        Channel::Email,
        "Change of address before next shipment",
        "We're moving offices on the 1st. Please make sure the next shipment goes to Keizersgracht 100, Amsterdam.",
    ),
    (
        "Contact form",
        Channel::WebForm,
        "Question about family subscription",
        "Do you offer a family plan? We'd need 4 seats. Couldn't find anything on the pricing page.",
    ),
    (
        "AppStore review",
        Channel::Review,
        "★★★★★ Support turned it around",
        "Had an issue with my order but support fixed it within a day. That's how you keep customers!",
    ),
    (
        "Ticket #7723",
        Channel::Ticket,
        "Urgent: account locked out, demo in 1 hour",
        "Our whole team is locked out after the SSO change and we have a customer demo at 15:00. Please help ASAP.",
    ),
    (
        "p.bakker@example.com",
        Channel::Email,
        "Cancel my subscription",
        "I'd like to cancel effective next month. Nothing wrong with the service, we just no longer need it.",
    ),
    (
        "Ticket #7724",
        Channel::Ticket,
        "Webhook retries flooding our endpoint",
        "Your webhook retry policy is hammering our server 50x per minute for one failed delivery. Can you cap it?",
    ),
];

/// Deterministic mock inbox for small fixtures: `count` open messages,
/// oldest first. Gameplay uses `generate_course`; tests still lean on this.
#[allow(dead_code)] // exercised by track.rs tests; kept as a stable fixture
pub fn sample_messages(count: usize) -> Vec<Message> {
    let base_ts: i64 = BASE_TS;
    (0..count)
        .map(|i| {
            let (sender, channel, subject, body) = SAMPLES[i % SAMPLES.len()];
            Message {
                id: i as u64 + 1,
                channel,
                sender: sender.to_string(),
                subject: subject.to_string(),
                body: body.to_string(),
                received_at: base_ts + (i as i64) * 540,
                status: MessageStatus::Open,
            }
        })
        .collect()
}

/// One extra message "arriving" mid-run, for the new-hurdle-drops-in
/// scenario. Deterministic per resulting id, varied via the generator.
pub fn next_incoming(existing: &[Message]) -> Message {
    let id = existing.iter().map(|m| m.id).max().unwrap_or(0) + 1;
    let spec = CourseSpec {
        seed: 0xF1E2_D3C4 ^ id,
        difficulty: Difficulty::NormalShift,
        count: 1,
    };
    let mut msg = generate_course(&spec).pop().expect("count >= 1");
    msg.id = id;
    msg.received_at = existing
        .iter()
        .map(|m| m.received_at)
        .max()
        .unwrap_or(BASE_TS)
        + 300;
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::{self, Sentiment, Urgency};

    fn spec(seed: u64, difficulty: Difficulty) -> CourseSpec {
        CourseSpec {
            seed,
            difficulty,
            count: COURSE_LEN,
        }
    }

    fn fingerprint(course: &[Message]) -> String {
        course
            .iter()
            .map(|m| format!("{}|{}|{}|{}\n", m.id, m.sender, m.subject, m.received_at))
            .collect()
    }

    #[test]
    fn same_seed_same_course() {
        let a = generate_course(&spec(42, Difficulty::NormalShift));
        let b = generate_course(&spec(42, Difficulty::NormalShift));
        assert_eq!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn different_seeds_differ() {
        let a = generate_course(&spec(42, Difficulty::NormalShift));
        let b = generate_course(&spec(43, Difficulty::NormalShift));
        assert_ne!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn course_is_large_and_varied() {
        // The default demo course itself must show the full variety.
        let course = generate_course(&spec(DEFAULT_SEED, Difficulty::NormalShift));
        assert!(course.len() >= 50);
        // Unique ascending ids, nondecreasing timestamps, all open.
        for (i, m) in course.iter().enumerate() {
            assert_eq!(m.id, i as u64 + 1);
            assert_eq!(m.status, MessageStatus::Open);
            if i > 0 {
                assert!(m.received_at >= course[i - 1].received_at);
            }
        }
        // All four channels appear.
        for channel in shared::Channel::ALL {
            assert!(course.iter().any(|m| m.channel == channel), "{channel:?} missing");
        }
        // Mixed languages: some Dutch messages are present.
        let dutch = ["jullie", "graag", "bestelling", "wachtwoord", "geholpen"];
        assert!(course
            .iter()
            .any(|m| dutch.iter().any(|w| m.body.to_lowercase().contains(w))));
        // Mixed moods and urgencies emerge from enrichment.
        let metas: Vec<_> = course.iter().map(meta::enrich).collect();
        assert!(metas.iter().any(|m| m.sentiment == Sentiment::Positive));
        assert!(metas.iter().any(|m| m.sentiment != Sentiment::Positive));
        assert!(metas.iter().any(|m| m.urgency >= Urgency::High));
        assert!(metas.iter().any(|m| m.urgency <= Urgency::Normal));
    }

    #[test]
    fn presets_change_urgency_and_sentiment_mix() {
        let count = |d: Difficulty| {
            let course = generate_course(&spec(42, d));
            let metas: Vec<_> = course.iter().map(meta::enrich).collect();
            let hot = metas.iter().filter(|m| m.urgency >= Urgency::High).count();
            let angry = metas.iter().filter(|m| m.sentiment == Sentiment::Angry).count();
            let happy = metas
                .iter()
                .filter(|m| m.sentiment == Sentiment::Positive)
                .count();
            (hot, angry, happy)
        };
        let (chill_hot, chill_angry, chill_happy) = count(Difficulty::ChillJog);
        let (night_hot, night_angry, night_happy) = count(Difficulty::NightmareMonday);
        assert!(night_hot > chill_hot, "{night_hot} vs {chill_hot}");
        assert!(night_angry > chill_angry, "{night_angry} vs {chill_angry}");
        assert!(chill_happy > night_happy, "{chill_happy} vs {night_happy}");
    }

    #[test]
    fn reset_flag_relays_a_fresh_course() {
        let old = spec(42, Difficulty::NormalShift);
        let new = old.reseeded();
        assert_ne!(old.seed, new.seed);
        assert_eq!(old.difficulty, new.difficulty);
        let relaid = generate_course(&new);
        assert_eq!(relaid.len(), COURSE_LEN);
        assert!(relaid.iter().all(|m| m.status == MessageStatus::Open));
        assert_ne!(
            fingerprint(&generate_course(&old)),
            fingerprint(&relaid)
        );
    }

    #[test]
    fn next_incoming_is_deterministic_and_extends_the_course() {
        let course = generate_course(&spec(42, Difficulty::NormalShift));
        let a = next_incoming(&course);
        let b = next_incoming(&course);
        assert_eq!(a.id, course.len() as u64 + 1);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        assert!(a.received_at > course.last().unwrap().received_at);
    }
}
