# The Track — Every Message Is a Hurdle

## Story
As a support agent playing on my phone, I want all incoming messages (emails, forms, reviews, tickets) to appear as hurdles on a running track, so that working the queue feels like a run I want to finish rather than a pile I have to dig through.

## Requirements

### Requirement: Track view
The system SHALL render all open messages as hurdles placed along a scrollable track in a mobile-first, one-thumb-playable UI, with each hurdle styled by its source channel, and SHALL move my runner avatar to the next hurdle when I clear one.

#### Scenario: Messages become hurdles
- **WHEN** messages arrive via email, a web form, a review platform, and a ticket system
- **THEN** each appears as a hurdle on the track with a channel-specific look (e.g. envelope, star, ticket)
- **AND** the track shows how many hurdles remain until the finish line

#### Scenario: New hurdle drops onto the track
- **WHEN** a new message is ingested mid-run
- **THEN** a new hurdle animates onto the track ahead of the runner without a page reload

#### Scenario: One-thumb play
- **WHEN** the game is opened on a phone
- **THEN** all core actions (approach, clear, skip) are reachable with swipe and tap gestures in one-handed use
