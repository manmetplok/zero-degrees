# Taking the Hurdle — Review, Send, Clear

## Story
As a player, I want clearing a hurdle to mean reviewing and sending a real reply — with a satisfying jump animation when I do — so that the game action and the actual customer outcome are always the same thing.

## Requirements

### Requirement: Human-approved clears
The system SHALL require explicit player approval before any reply is sent, SHALL allow free editing of AI drafts, SHALL mark the message resolved when sent, and SHALL play the hurdle-clear animation and score award only at that moment.

#### Scenario: Player edits and clears
- **WHEN** a player edits the draft and swipes up to send
- **THEN** the reply is sent (or simulated in the demo), the message resolves, the runner jumps the hurdle, and points are awarded

#### Scenario: No auto-jump
- **WHEN** an AI draft is generated
- **THEN** nothing is sent and no hurdle is cleared until the player explicitly approves

#### Scenario: Balk
- **WHEN** a player backs out of a hurdle without sending
- **THEN** the message stays open on the track with no penalty to its data, and any draft is kept
