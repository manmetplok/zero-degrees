# Hazard Zones — Recurring Theme Detection

## Story
As a team lead, I want the AI to mark stretches of track as hazard zones when many hurdles share a root cause, so that we spot "the checkout page is broken" as one fixable problem instead of fifty separate jumps.

## Requirements

### Requirement: Theme clustering as hazards
The system SHALL periodically analyze open and recent messages with an AI model to detect recurring themes, and SHALL render each theme as a named hazard zone (with description and message count) grouping its hurdles on the track and map.

#### Scenario: Spike becomes a hazard zone
- **WHEN** many messages about the same underlying issue arrive in a short period
- **THEN** a hazard zone appears (e.g. "⚠ Checkout failures — 14 hurdles") grouping the related hurdles

#### Scenario: Entering the zone
- **WHEN** a lead taps a hazard zone
- **THEN** the view filters to only that zone's hurdles
- **AND** an AI-written zone briefing summarizes the common root cause for whoever takes it on
