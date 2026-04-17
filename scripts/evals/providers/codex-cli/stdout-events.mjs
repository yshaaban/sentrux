import { nowMs } from './shared.mjs';
import { parseJsonLine } from './json-lines.mjs';

function summarizeCommandItem(item) {
  if (!item || item.type !== 'command_execution') {
    return null;
  }

  return {
    command: typeof item.command === 'string' ? item.command : null,
    exit_code: Number.isInteger(item.exit_code) ? item.exit_code : null,
    status: typeof item.status === 'string' ? item.status : null,
  };
}

function createStdoutEventSummary() {
  return {
    event_count: 0,
    parsed_event_count: 0,
    unparsed_line_count: 0,
    turn_count: 0,
    command_started_count: 0,
    command_completed_count: 0,
    completed_item_count: 0,
    agent_message_count: 0,
    last_event_type: null,
    last_event_elapsed_ms: null,
    last_completed_item_type: null,
    last_completed_item_elapsed_ms: null,
    last_started_command: null,
    last_started_command_elapsed_ms: null,
    in_progress_command: null,
    in_progress_command_elapsed_ms: null,
    last_completed_command: null,
    last_completed_command_elapsed_ms: null,
    last_completed_command_exit_code: null,
    last_completed_command_status: null,
    last_agent_message: null,
    last_agent_message_elapsed_ms: null,
  };
}

function elapsedSince(startedMs) {
  return Number((nowMs() - startedMs).toFixed(1));
}

function updateStartedCommand(summary, item, eventElapsedMs) {
  const command = summarizeCommandItem(item);

  summary.command_started_count += 1;
  summary.last_started_command = command;
  summary.last_started_command_elapsed_ms = eventElapsedMs;
  summary.in_progress_command = command;
  summary.in_progress_command_elapsed_ms = eventElapsedMs;
}

function updateCompletedCommand(summary, item, eventElapsedMs) {
  const command = summarizeCommandItem(item);

  summary.command_completed_count += 1;
  summary.last_completed_command = command;
  summary.last_completed_command_elapsed_ms = eventElapsedMs;
  summary.last_completed_command_exit_code = command?.exit_code ?? null;
  summary.last_completed_command_status = command?.status ?? null;
  summary.in_progress_command = null;
  summary.in_progress_command_elapsed_ms = null;
}

function updateAgentMessage(summary, item, eventElapsedMs) {
  summary.agent_message_count += 1;
  summary.last_agent_message = typeof item.text === 'string' ? item.text : null;
  summary.last_agent_message_elapsed_ms = eventElapsedMs;
}

function consumeStdoutEventLine(summary, line, startedMs) {
  if (!line.trim()) {
    return;
  }

  summary.event_count += 1;
  const parsed = parseJsonLine(line);
  if (!parsed) {
    summary.unparsed_line_count += 1;
    return;
  }

  const eventElapsedMs = elapsedSince(startedMs);
  summary.parsed_event_count += 1;
  summary.last_event_type = typeof parsed.type === 'string' ? parsed.type : null;
  summary.last_event_elapsed_ms = eventElapsedMs;

  if (parsed.type === 'turn.started') {
    summary.turn_count += 1;
    return;
  }

  const item = parsed.item ?? null;
  const itemType = item?.type;
  if (typeof itemType !== 'string') {
    return;
  }

  if (parsed.type === 'item.started' && itemType === 'command_execution') {
    updateStartedCommand(summary, item, eventElapsedMs);
    return;
  }
  if (parsed.type !== 'item.completed') {
    return;
  }

  summary.completed_item_count += 1;
  summary.last_completed_item_type = itemType;
  summary.last_completed_item_elapsed_ms = eventElapsedMs;

  if (itemType === 'command_execution') {
    updateCompletedCommand(summary, item, eventElapsedMs);
    return;
  }
  if (itemType === 'agent_message') {
    updateAgentMessage(summary, item, eventElapsedMs);
  }
}

export function createStdoutEventTracker() {
  let remainder = '';
  const summary = createStdoutEventSummary();
  const startedMs = nowMs();

  function consume(text) {
    remainder += text;
    const lines = remainder.split(/\r?\n/);
    remainder = lines.pop() ?? '';
    for (const line of lines) {
      consumeStdoutEventLine(summary, line, startedMs);
    }
  }

  function finish() {
    if (remainder) {
      consumeStdoutEventLine(summary, remainder, startedMs);
      remainder = '';
    }
  }

  function snapshot() {
    return JSON.parse(JSON.stringify(summary));
  }

  return {
    consume,
    finish,
    snapshot,
  };
}
