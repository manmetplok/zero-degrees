# Coaching the AI — Suggestion Feedback

## Story
As a player, I want to rate the AI's hurdle typing, heights, scout reports, and reply power-ups with a quick thumbs up or down, so that the team can see where our AI coach helps and where it trips us up.

## Requirements

### Requirement: Feedback on AI output
The system SHALL let players rate each AI-generated category, urgency, summary, and draft reply as helpful or unhelpful with one tap, SHALL persist the feedback together with the AI output and the player's final version, and SHALL show aggregate accuracy per AI feature on race control.

#### Scenario: Rating a power-up draft
- **WHEN** a player thumbs-down an AI draft before rewriting it
- **THEN** the rating, the original draft, and the player's sent version are stored together

#### Scenario: Coach report
- **WHEN** a team lead opens race control
- **THEN** they can see helpful/unhelpful ratios per AI feature (typing, heights, scout reports, drafts)
- **AND** the trend over time shows whether prompt tweaks are improving the AI's game
