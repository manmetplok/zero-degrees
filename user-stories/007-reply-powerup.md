# Reply Power-Up — AI Draft Replies

## Story
As a player at a hurdle, I want to trigger an AI power-up that drafts my reply, so that I can clear hurdles fast and consistently instead of typing every answer from scratch on a phone keyboard.

## Requirements

### Requirement: Draft power-up
The system SHALL offer a "power-up" action on each hurdle that calls an AI model to draft a reply addressing the message's content, matching Meridian's tone of voice, and written in the language of the original message.

#### Scenario: Power-up on a complaint
- **WHEN** a player activates the power-up on a flame-aura complaint hurdle
- **THEN** the AI drafts an empathetic reply that acknowledges the issue and proposes a concrete next step
- **AND** the draft lands in an editable reply field with a power-up charge animation

#### Scenario: Language match
- **WHEN** the original message is in Dutch
- **THEN** the drafted reply is in Dutch

#### Scenario: Recharge
- **WHEN** a draft misses the mark and the player taps "recharge"
- **THEN** the AI generates a new draft taking the player's short steering note into account
