use crate::messages::NewMessage;
use rand::distributions::{Distribution, WeightedIndex};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use shared::{Channel, Difficulty, Sentiment, Urgency};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait CourseGenerator: Send + Sync {
    fn generate(&self, difficulty: Difficulty, count: usize, seed: u64) -> Vec<NewMessage>;
}

pub fn default_generator() -> Box<dyn CourseGenerator> {
    Box::new(MockCourseGenerator)
}

pub struct MockCourseGenerator;

struct Template {
    language: &'static str,
    subject: &'static str,
    body: &'static str,
}

const TEMPLATES: &[Template] = &[
    Template { language: "en", subject: "Incorrect amount on invoice", body: "My invoice for last month shows an incorrect amount." },
    Template { language: "en", subject: "Package still not delivered", body: "My package has not arrived and tracking has not updated in five days." },
    Template { language: "en", subject: "App crashes on photo upload", body: "The app crashes every time I try to upload a photo." },
    Template { language: "en", subject: "Dark mode request", body: "It would be great if the app supported dark mode." },
    Template { language: "en", subject: "Refund request for order", body: "I would like to request a refund for my last order." },
    Template { language: "en", subject: "Can't access my account", body: "I can't log into my account even after resetting my password." },
    Template { language: "en", subject: "Complaint about support call", body: "The support agent I spoke to yesterday was very rude." },
    Template { language: "en", subject: "How to change subscription plan", body: "How do I change my subscription plan?" },
    Template { language: "en", subject: "Cancel my subscription", body: "I want to cancel my subscription effective immediately." },
    Template { language: "en", subject: "Thank you for the quick fix", body: "Your team resolved my issue so quickly, I'm impressed." },
    Template { language: "nl", subject: "Onjuist bedrag op factuur", body: "Mijn factuur van vorige maand toont een onjuist bedrag." },
    Template { language: "nl", subject: "Pakket nog niet aangekomen", body: "Mijn pakket is niet aangekomen en de tracking is al vijf dagen niet bijgewerkt." },
    Template { language: "nl", subject: "App crasht bij foto uploaden", body: "De app crasht elke keer dat ik een foto probeer te uploaden." },
    Template { language: "nl", subject: "Verzoek om donkere modus", body: "Het zou geweldig zijn als de app een donkere modus had." },
    Template { language: "nl", subject: "Terugbetaling voor bestelling", body: "Ik wil graag een terugbetaling aanvragen voor mijn laatste bestelling." },
    Template { language: "nl", subject: "Kan niet inloggen op mijn account", body: "Ik kan niet inloggen op mijn account, ook niet na het resetten van mijn wachtwoord." },
    Template { language: "nl", subject: "Klacht over supportgesprek", body: "De medewerker die ik gisteren aan de lijn had, was erg onbeleefd." },
    Template { language: "nl", subject: "Hoe wijzig ik mijn abonnement", body: "Hoe kan ik mijn abonnement wijzigen?" },
    Template { language: "nl", subject: "Abonnement opzeggen", body: "Ik wil mijn abonnement per direct opzeggen." },
    Template { language: "nl", subject: "Bedankt voor de snelle oplossing", body: "Jullie team heeft mijn probleem zo snel opgelost, erg onder de indruk." },
    Template { language: "de", subject: "Falscher Betrag auf der Rechnung", body: "Meine Rechnung vom letzten Monat zeigt einen falschen Betrag." },
    Template { language: "de", subject: "Paket noch nicht angekommen", body: "Mein Paket ist nicht angekommen und die Sendungsverfolgung wurde seit fuenf Tagen nicht aktualisiert." },
    Template { language: "de", subject: "App stuerzt beim Foto-Upload ab", body: "Die App stuerzt jedes Mal ab, wenn ich versuche, ein Foto hochzuladen." },
    Template { language: "de", subject: "Anfrage fuer Dunkelmodus", body: "Es waere grossartig, wenn die App einen Dunkelmodus haette." },
    Template { language: "de", subject: "Rueckerstattung fuer Bestellung", body: "Ich moechte eine Rueckerstattung fuer meine letzte Bestellung beantragen." },
    Template { language: "de", subject: "Kann nicht auf mein Konto zugreifen", body: "Ich kann mich nicht in mein Konto einloggen, selbst nach dem Zuruecksetzen meines Passworts." },
    Template { language: "de", subject: "Beschwerde ueber Support-Anruf", body: "Der Mitarbeiter, mit dem ich gestern gesprochen habe, war sehr unfreundlich." },
    Template { language: "de", subject: "Wie aendere ich mein Abonnement", body: "Wie kann ich meinen Tarif aendern?" },
    Template { language: "de", subject: "Abonnement kuendigen", body: "Ich moechte mein Abonnement sofort kuendigen." },
    Template { language: "de", subject: "Danke fuer die schnelle Loesung", body: "Ihr Team hat mein Problem so schnell geloest, das hat mich beeindruckt." },
    Template { language: "fr", subject: "Montant incorrect sur la facture", body: "Ma facture du mois dernier affiche un montant incorrect." },
    Template { language: "fr", subject: "Colis toujours pas livre", body: "Mon colis n'est pas arrive et le suivi n'a pas ete mis a jour depuis cinq jours." },
    Template { language: "fr", subject: "L'application plante lors de l'envoi de photos", body: "L'application plante chaque fois que j'essaie de telecharger une photo." },
    Template { language: "fr", subject: "Demande de mode sombre", body: "Ce serait super si l'application avait un mode sombre." },
    Template { language: "fr", subject: "Demande de remboursement", body: "Je souhaite demander un remboursement pour ma derniere commande." },
    Template { language: "fr", subject: "Impossible d'acceder a mon compte", body: "Je ne peux pas me connecter a mon compte, meme apres avoir reinitialise mon mot de passe." },
    Template { language: "fr", subject: "Plainte concernant un appel au support", body: "L'agent avec qui j'ai parle hier etait tres impoli." },
    Template { language: "fr", subject: "Comment changer mon abonnement", body: "Comment puis-je changer mon abonnement ?" },
    Template { language: "fr", subject: "Annuler mon abonnement", body: "Je souhaite annuler mon abonnement immediatement." },
    Template { language: "fr", subject: "Merci pour la resolution rapide", body: "Votre equipe a resolu mon probleme si vite, je suis impressionne." },
    Template { language: "es", subject: "Importe incorrecto en la factura", body: "Mi factura del mes pasado muestra un importe incorrecto." },
    Template { language: "es", subject: "El paquete todavia no ha llegado", body: "Mi paquete no ha llegado y el seguimiento no se ha actualizado en cinco dias." },
    Template { language: "es", subject: "La aplicacion falla al subir fotos", body: "La aplicacion falla cada vez que intento subir una foto." },
    Template { language: "es", subject: "Solicitud de modo oscuro", body: "Seria genial que la aplicacion tuviera un modo oscuro." },
    Template { language: "es", subject: "Solicitud de reembolso", body: "Quisiera solicitar un reembolso por mi ultimo pedido." },
    Template { language: "es", subject: "No puedo acceder a mi cuenta", body: "No puedo iniciar sesion en mi cuenta, incluso despues de restablecer mi contrasena." },
    Template { language: "es", subject: "Queja sobre una llamada de soporte", body: "El agente con quien hable ayer fue muy grosero." },
    Template { language: "es", subject: "Como cambiar mi plan de suscripcion", body: "Como puedo cambiar mi plan de suscripcion?" },
    Template { language: "es", subject: "Cancelar mi suscripcion", body: "Quiero cancelar mi suscripcion de inmediato." },
    Template { language: "es", subject: "Gracias por la rapida solucion", body: "Su equipo resolvio mi problema tan rapido, estoy impresionado." },
];

const SENDER_NAMES: &[&str] = &[
    "Alex Morgan",
    "Jamie Chen",
    "Sam O'Connor",
    "Priya Patel",
    "Lukas Becker",
    "Fatima Al-Sayed",
    "Noah Kim",
    "Elena Rossi",
    "Mateo Garcia",
    "Sophie Laurent",
    "Ravi Nair",
    "Anna Kowalski",
    "Oliver Smith",
    "Yuki Tanaka",
    "Ines Dubois",
    "Tobias Meyer",
];

const URGENCIES: [Urgency; 4] = [
    Urgency::Low,
    Urgency::Normal,
    Urgency::High,
    Urgency::Critical,
];

const SENTIMENTS: [Sentiment; 4] = [
    Sentiment::Positive,
    Sentiment::Neutral,
    Sentiment::Negative,
    Sentiment::Angry,
];

fn urgency_weights(difficulty: Difficulty) -> [u32; 4] {
    match difficulty {
        Difficulty::ChillJog => [50, 35, 12, 3],
        Difficulty::NormalShift => [25, 40, 25, 10],
        Difficulty::NightmareMonday => [5, 20, 40, 35],
    }
}

fn sentiment_weights(difficulty: Difficulty) -> [u32; 4] {
    match difficulty {
        Difficulty::ChillJog => [45, 40, 10, 5],
        Difficulty::NormalShift => [20, 40, 25, 15],
        Difficulty::NightmareMonday => [5, 20, 35, 40],
    }
}

fn tone_phrase(language: &str, sentiment: Sentiment) -> &'static str {
    match (language, sentiment) {
        ("en", Sentiment::Angry) => " This is honestly infuriating.",
        ("en", Sentiment::Negative) => " I'm quite disappointed.",
        ("en", Sentiment::Positive) => " Really happy with the service.",
        ("nl", Sentiment::Angry) => " Dit is echt onacceptabel.",
        ("nl", Sentiment::Negative) => " Ik ben hier teleurgesteld over.",
        ("nl", Sentiment::Positive) => " Erg blij met de service.",
        ("de", Sentiment::Angry) => " Das ist wirklich inakzeptabel.",
        ("de", Sentiment::Negative) => " Ich bin davon enttaeuscht.",
        ("de", Sentiment::Positive) => " Sehr zufrieden mit dem Service.",
        ("fr", Sentiment::Angry) => " C'est vraiment inacceptable.",
        ("fr", Sentiment::Negative) => " Je suis assez decu.",
        ("fr", Sentiment::Positive) => " Tres content du service.",
        ("es", Sentiment::Angry) => " Esto es realmente inaceptable.",
        ("es", Sentiment::Negative) => " Estoy bastante decepcionado.",
        ("es", Sentiment::Positive) => " Muy contento con el servicio.",
        _ => "",
    }
}

fn slugify(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ')
        .collect::<String>()
        .to_lowercase()
        .replace(' ', ".")
}

fn sender_for(channel: Channel, name: &str, rng: &mut StdRng) -> String {
    match channel {
        Channel::Email => format!("{} <{}@example.com>", name, slugify(name)),
        Channel::WebForm => name.to_string(),
        Channel::Review => format!("{} (verified purchase)", name),
        Channel::Ticket => format!("{} - ticket-{:04}", name, rng.gen_range(1000..9999)),
    }
}

impl CourseGenerator for MockCourseGenerator {
    fn generate(&self, difficulty: Difficulty, count: usize, seed: u64) -> Vec<NewMessage> {
        let mut rng = StdRng::seed_from_u64(seed);
        let urgency_dist =
            WeightedIndex::new(urgency_weights(difficulty)).expect("static urgency weights");
        let sentiment_dist =
            WeightedIndex::new(sentiment_weights(difficulty)).expect("static sentiment weights");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut templates: Vec<&Template> = TEMPLATES.iter().collect();
        templates.shuffle(&mut rng);

        (0..count)
            .map(|i| {
                let template = templates[i % templates.len()];
                let channel = Channel::ALL[rng.gen_range(0..Channel::ALL.len())];
                let name = SENDER_NAMES[rng.gen_range(0..SENDER_NAMES.len())];
                let urgency = URGENCIES[urgency_dist.sample(&mut rng)];
                let sentiment = SENTIMENTS[sentiment_dist.sample(&mut rng)];
                let received_at = now - rng.gen_range(0..259_200);
                let body = format!(
                    "{}{}",
                    template.body,
                    tone_phrase(template.language, sentiment)
                );
                NewMessage {
                    channel,
                    sender: sender_for(channel, name, &mut rng),
                    subject: template.subject.to_string(),
                    body,
                    received_at,
                    urgency,
                    sentiment,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urgency_severity_ratio(messages: &[NewMessage]) -> f64 {
        let severe = messages
            .iter()
            .filter(|m| matches!(m.urgency, Urgency::High | Urgency::Critical))
            .count();
        severe as f64 / messages.len() as f64
    }

    fn positive_ratio(messages: &[NewMessage]) -> f64 {
        let positive = messages
            .iter()
            .filter(|m| matches!(m.sentiment, Sentiment::Positive | Sentiment::Neutral))
            .count();
        positive as f64 / messages.len() as f64
    }

    #[test]
    fn generate_returns_requested_count() {
        let generator = MockCourseGenerator;
        let messages = generator.generate(Difficulty::NormalShift, 75, 1);
        assert_eq!(messages.len(), 75);
    }

    #[test]
    fn generate_is_deterministic_for_same_seed() {
        let generator = MockCourseGenerator;
        let first = generator.generate(Difficulty::NormalShift, 60, 42);
        let second = generator.generate(Difficulty::NormalShift, 60, 42);
        assert_eq!(
            first.iter().map(|m| (&m.subject, &m.body)).collect::<Vec<_>>(),
            second.iter().map(|m| (&m.subject, &m.body)).collect::<Vec<_>>()
        );
        assert_eq!(
            first.iter().map(|m| m.urgency).collect::<Vec<_>>(),
            second.iter().map(|m| m.urgency).collect::<Vec<_>>()
        );
    }

    #[test]
    fn nightmare_monday_skews_more_urgent_than_chill_jog() {
        let generator = MockCourseGenerator;
        let chill = generator.generate(Difficulty::ChillJog, 200, 7);
        let nightmare = generator.generate(Difficulty::NightmareMonday, 200, 7);
        assert!(urgency_severity_ratio(&nightmare) > urgency_severity_ratio(&chill));
    }

    #[test]
    fn chill_jog_skews_more_positive_than_nightmare_monday() {
        let generator = MockCourseGenerator;
        let chill = generator.generate(Difficulty::ChillJog, 200, 7);
        let nightmare = generator.generate(Difficulty::NightmareMonday, 200, 7);
        assert!(positive_ratio(&chill) > positive_ratio(&nightmare));
    }
}
