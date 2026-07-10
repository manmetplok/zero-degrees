# Track Order — Priority Decides the Course

## Story
As a player, I want the track laid out by AI-derived priority instead of arrival time, so that just playing forward means I'm always taking the most important hurdle next.

## Requirements

### Requirement: Priority-ordered track
The system SHALL compute each open message's priority from urgency, sentiment, and age, and SHALL place hurdles on the track in priority order with the highest priority nearest the runner.

#### Scenario: Critical hurdle cuts in
- **WHEN** a new message is classified as critical
- **THEN** its hurdle drops onto the track directly in front of the runner, ahead of older low hurdles

#### Scenario: Waiting hurdles creep closer
- **WHEN** a normal message stays unanswered much longer than others
- **THEN** its priority rises and its hurdle visibly moves up the track

#### Scenario: Free-run mode
- **WHEN** a player switches to free-run mode
- **THEN** they may take hurdles in any order, but skipped high hurdles remain marked as "ahead of you"
