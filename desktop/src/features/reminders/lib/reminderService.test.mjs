import assert from "node:assert/strict";
import test from "node:test";

import { parseNotBefore, parseReminderContent } from "./reminderService.ts";

const VALID_TARGET = {
  eventId: "abc123",
  channelId: "chan1",
  preview: "hello world",
  authorPubkey: "pk1",
};

function content(overrides = {}) {
  return JSON.stringify({
    target: VALID_TARGET,
    status: "pending",
    ...overrides,
  });
}

test("parseReminderContent_valid_target_reminder_returns_content", () => {
  const result = parseReminderContent(content());
  assert.deepEqual(result, {
    status: "pending",
    target: VALID_TARGET,
    note: undefined,
  });
});

test("parseReminderContent_note_only_reminder_returns_content", () => {
  const result = parseReminderContent(
    JSON.stringify({ status: "pending", note: "buy milk" }),
  );
  assert.deepEqual(result, {
    status: "pending",
    target: undefined,
    note: "buy milk",
  });
});

test("parseReminderContent_done_and_cancelled_statuses_accepted", () => {
  assert.equal(
    parseReminderContent(content({ status: "done" }))?.status,
    "done",
  );
  assert.equal(
    parseReminderContent(content({ status: "cancelled" }))?.status,
    "cancelled",
  );
});

test("parseReminderContent_invalid_json_returns_null", () => {
  assert.equal(parseReminderContent("not json {"), null);
});

test("parseReminderContent_non_object_returns_null", () => {
  assert.equal(parseReminderContent("42"), null);
  assert.equal(parseReminderContent('"a string"'), null);
  assert.equal(parseReminderContent("[1,2,3]"), null);
  assert.equal(parseReminderContent("null"), null);
});

test("parseReminderContent_unknown_status_returns_null", () => {
  assert.equal(parseReminderContent(content({ status: "bogus" })), null);
  assert.equal(
    parseReminderContent(JSON.stringify({ target: VALID_TARGET })),
    null,
  );
});

test("parseReminderContent_neither_target_nor_note_returns_null", () => {
  assert.equal(
    parseReminderContent(JSON.stringify({ status: "pending" })),
    null,
  );
});

test("parseReminderContent_empty_note_without_target_returns_null", () => {
  assert.equal(
    parseReminderContent(JSON.stringify({ status: "pending", note: "" })),
    null,
  );
});

test("parseReminderContent_non_string_note_returns_null", () => {
  assert.equal(parseReminderContent(content({ note: 5 })), null);
});

test("parseReminderContent_malformed_target_returns_null", () => {
  assert.equal(
    parseReminderContent(JSON.stringify({ status: "pending", target: {} })),
    null,
  );
  assert.equal(
    parseReminderContent(
      JSON.stringify({ status: "pending", target: "string" }),
    ),
    null,
  );
  assert.equal(
    parseReminderContent(
      JSON.stringify({
        status: "pending",
        target: { ...VALID_TARGET, preview: 7 },
      }),
    ),
    null,
  );
});

test("parseReminderContent_unknown_fields_are_ignored", () => {
  const result = parseReminderContent(content({ extra: "ignored" }));
  assert.deepEqual(result, {
    status: "pending",
    target: VALID_TARGET,
    note: undefined,
  });
});

// --- parseTarget partial target support ---

test("parseReminderContent_target_with_only_channelId_is_valid", () => {
  const result = parseReminderContent(
    JSON.stringify({
      status: "pending",
      target: { channelId: "chan1" },
      note: "test",
    }),
  );
  assert.ok(result);
  assert.deepEqual(result?.target, {
    eventId: "",
    channelId: "chan1",
    preview: "",
    authorPubkey: "",
  });
});

test("parseReminderContent_target_with_only_eventId_is_valid", () => {
  const result = parseReminderContent(
    JSON.stringify({
      status: "pending",
      target: { eventId: "evt123" },
      note: "test",
    }),
  );
  assert.ok(result);
  assert.deepEqual(result?.target, {
    eventId: "evt123",
    channelId: "",
    preview: "",
    authorPubkey: "",
  });
});

test("parseReminderContent_target_with_both_ids_missing_preview_author_is_valid", () => {
  const result = parseReminderContent(
    JSON.stringify({
      status: "pending",
      target: { eventId: "evt123", channelId: "chan1" },
      note: "test",
    }),
  );
  assert.ok(result);
  assert.deepEqual(result?.target, {
    eventId: "evt123",
    channelId: "chan1",
    preview: "",
    authorPubkey: "",
  });
});

test("parseReminderContent_empty_target_object_returns_null", () => {
  // Empty target object has neither eventId nor channelId
  const result = parseReminderContent(
    JSON.stringify({ status: "pending", target: {} }),
  );
  assert.equal(result, null);
});

test("parseReminderContent_target_with_non_string_fields_coerces_to_empty", () => {
  const result = parseReminderContent(
    JSON.stringify({
      status: "pending",
      target: { channelId: "chan1", preview: 123, authorPubkey: true },
      note: "test",
    }),
  );
  assert.ok(result);
  assert.deepEqual(result?.target, {
    eventId: "",
    channelId: "chan1",
    preview: "",
    authorPubkey: "",
  });
});

test("parseNotBefore_valid_digits_returns_number", () => {
  assert.equal(parseNotBefore("0"), 0);
  assert.equal(parseNotBefore("1700000000"), 1_700_000_000);
});

test("parseNotBefore_leading_zero_returns_undefined", () => {
  assert.equal(parseNotBefore("007"), undefined);
  assert.equal(parseNotBefore("01"), undefined);
});

test("parseNotBefore_non_digit_returns_undefined", () => {
  assert.equal(parseNotBefore("123abc"), undefined);
  assert.equal(parseNotBefore("12.5"), undefined);
  assert.equal(parseNotBefore("-5"), undefined);
  assert.equal(parseNotBefore(" 5"), undefined);
  assert.equal(parseNotBefore(""), undefined);
});

test("parseNotBefore_above_max_safe_integer_returns_undefined", () => {
  assert.equal(parseNotBefore("9007199254740991"), 9_007_199_254_740_991);
  assert.equal(parseNotBefore("9007199254740992"), undefined);
});
