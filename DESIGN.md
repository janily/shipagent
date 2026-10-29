# Workbench native design context

## Overview
The uploaded five-screen reference and docs/spec/04 are authoritative. This is a native developer Agent workbench, not a marketing surface. Preserve the quiet compact sidebar, tool activity rows, unboxed assistant Markdown, right-aligned user bubbles and bottom composer.

## Colors
Canonical runtime tokens are GPUI Kit Theme; components use ActiveTheme roles: background/foreground, sidebar/sidebar_foreground, muted/muted_foreground, border, primary/primary_foreground, warning and destructive. No screen-local palette. Light/dark preserve semantic hierarchy.

## Typography
System sans for Chinese/English; Kit mono font for tool commands/results. Body 14px, auxiliary 12px, section title 16px as specified. Owned labels use rust-i18n ak.* keys in zh-CN/en resources; language selection initially uses AGENTKIT_LANGUAGE with zh-CN default.

## Layout
Default 1280x820, minimum 960x600; 40px titlebar, 24px statusbar; sidebar 280px; centered chat max820px; 24px minimum horizontal gutter. MessageScroller owns history scrolling; Textarea owns long draft scrolling. No placeholder chats or fabricated connection status.

## Components
Kit Button owns pointer/keyboard/disabled semantics; TextareaState owns IME and multiline editing; MessageScroller owns tail-following/virtualization; TextViewState owns incremental Markdown; Theme owns scrollbars. app controller owns RPC and sidecar lifecycle. docs/spec/04 is the maintained behavioral contract.

## Verification
Use headless gpui_kit::test for intents and streamed state, real backend/fake-model integration for Agent semantics, native screenshot for visual inspection. Lack of display/GPU or macOS host is recorded, not replaced by web previews.
