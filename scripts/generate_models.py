#!/usr/bin/env python3
"""Generate src/models_generated.rs from upstream models JSON.

Usage:
    # First dump models to JSON via bun:
    bun --eval "import { MODELS } from 'path/to/models.generated.js'; process.stdout.write(JSON.stringify(MODELS));" > /tmp/models.json
    # Then generate:
    python3 scripts/generate_models.py /tmp/models.json
"""

import json
import sys
import datetime
from pathlib import Path

VERIFIED_REASONING_EFFORTS = {"none", "low", "medium", "high", "xhigh", "max"}


def get_effort_thinking_level_map(controls):
    """Map model-dev reasoning controls to rs-ai/upstream thinkingLevelMap.

    Mirrors `scripts/models-dev-reasoning-options.ts`: only verified effort values
    produce a map; `none` disables thinking only when a toggle control is also present.
    """
    effort_values = None
    has_toggle = False
    for control in controls:
        if control.get("type") == "toggle":
            has_toggle = True
        if control.get("type") == "effort":
            effort_values = control.get("values")
    if not isinstance(effort_values, list):
        return None
    normalized = [value for value in effort_values if isinstance(value, str)]
    if len(normalized) != len(effort_values) or any(value not in VERIFIED_REASONING_EFFORTS for value in normalized):
        return None
    has = lambda value: value in normalized
    return {
        "off": "none" if has_toggle and has("none") else None,
        "minimal": None,
        "low": "low" if has("low") else None,
        "medium": "medium" if has("medium") else None,
        "high": "high" if has("high") else None,
        "xhigh": "xhigh" if has("xhigh") else None,
        "max": "max" if has("max") else None,
    }


def rust_string(s):
    return json.dumps(s)

def gen_model(m) -> str:
    lines = []
    lines.append("        Model {")
    lines.append(f'            id: {rust_string(m["id"])}.into(),')
    lines.append(f'            name: {rust_string(m["name"])}.into(),')
    lines.append(f'            api: {rust_string(m["api"])}.into(),')
    lines.append(f'            provider: {rust_string(m["provider"])}.into(),')
    lines.append(f'            base_url: {rust_string(m.get("baseUrl", ""))}.into(),')
    lines.append(f'            reasoning: {str(m.get("reasoning", False)).lower()},')
    
    tlm = m.get("thinkingLevelMap")
    if tlm:
        entries = []
        for k, v in tlm.items():
            if v is None:
                entries.append(f'                ({rust_string(k)}.into(), None)')
            else:
                entries.append(f'                ({rust_string(k)}.into(), Some({rust_string(v)}.into()))')
        lines.append("            thinking_level_map: Some(HashMap::from([")
        lines.append(",\n".join(entries))
        lines.append("            ])),")
    else:
        lines.append("            thinking_level_map: None,")
    
    inputs = m.get("input", [])
    input_str = ", ".join(f'{rust_string(i)}.into()' for i in inputs)
    lines.append(f"            input: vec![{input_str}],")
    
    cost = m.get("cost", {})
    ci = cost.get("input", 0)
    co = cost.get("output", 0)
    cr = cost.get("cacheRead", 0)
    cw = cost.get("cacheWrite", 0)
    tiers = cost.get("tiers", []) or []
    if tiers:
        tier_items = ", ".join(
            "ModelCostTier {{ input_tokens_above: {ta}_u64, input: {i}_f64, output: {o}_f64, cache_read: {r}_f64, cache_write: {w}_f64 }}".format(
                ta=t.get("inputTokensAbove", 0),
                i=t.get("input", 0),
                o=t.get("output", 0),
                r=t.get("cacheRead", 0),
                w=t.get("cacheWrite", 0),
            )
            for t in tiers
        )
        tiers_str = f"vec![{tier_items}]"
    else:
        tiers_str = "vec![]"
    lines.append(f"            cost: ModelCost {{ input: {ci}_f64, output: {co}_f64, cache_read: {cr}_f64, cache_write: {cw}_f64, tiers: {tiers_str} }},")
    lines.append(f"            context_window: {m.get('contextWindow', 0)},")
    lines.append(f"            max_tokens: {m.get('maxTokens', 0)},")
    sampling_params = m.get("samplingParams")
    if sampling_params:
        lines.append(f'            sampling_params: Some(serde_json::from_str({rust_string(json.dumps(sampling_params))}).unwrap()),')
    else:
        lines.append("            sampling_params: None,")
    
    headers = m.get("headers")
    if headers:
        entries = ", ".join(f'({rust_string(k)}.into(), {rust_string(v)}.into())' for k, v in headers.items())
        lines.append(f"            headers: Some(HashMap::from([{entries}])),")
    else:
        lines.append("            headers: None,")
    
    lines.append("            api_key: None,")
    compat = m.get("compat") or {}
    compat_fields = {
        "allowEmptySignature": ("allow_empty_signature", "bool"),
        "forceAdaptiveThinking": ("force_adaptive_thinking", "bool"),
        "maxTokensField": ("max_tokens_field", "str"),
        "requiresReasoningContentOnAssistantMessages": ("requires_reasoning_content_on_assistant_messages", "bool"),
        "requiresToolResultName": ("requires_tool_result_name", "bool"),
        "requiresThinkingAsText": ("requires_thinking_as_text", "bool"),
        "cacheControlFormat": ("cache_control_format", "str"),
        "requiresAssistantAfterToolResult": ("requires_assistant_after_tool_result", "bool"),
        "sendSessionAffinityHeaders": ("send_session_affinity_headers", "bool"),
        "sendSessionIdHeader": ("send_session_id_header", "bool"),
        "supportsCacheControlOnTools": ("supports_cache_control_on_tools", "bool"),
        "supportsDeveloperRole": ("supports_developer_role", "bool"),
        "supportsEagerToolInputStreaming": ("supports_eager_tool_input_streaming", "bool"),
        "supportsLongCacheRetention": ("supports_long_cache_retention", "bool"),
        "supportsReasoningEffort": ("supports_reasoning_effort", "bool"),
        "supportsStore": ("supports_store", "bool"),
        "supportsUsageInStreaming": ("supports_usage_in_streaming", "bool"),
        "supportsFinishReason": ("supports_finish_reason", "bool"),
        "supportsStrictMode": ("supports_strict_mode", "bool"),
        "supportsOpenAIGrammarTools": ("supports_openai_grammar_tools", "bool"),
        "supportsAdditionalTools": ("supports_additional_tools", "bool"),
        "supportsTemperature": ("supports_temperature", "bool"),
        "supportsThinkingTokenBudget": ("supports_thinking_token_budget", "bool"),
        "thinkingFormat": ("thinking_format", "str"),
        "zaiToolStream": ("zai_tool_stream", "bool"),
    }
    compat_lines = []
    for js_key, (rs_key, kind) in compat_fields.items():
        if js_key in compat and compat[js_key] is not None:
            v = compat[js_key]
            if kind == "bool":
                compat_lines.append(f"                {rs_key}: Some({str(v).lower()}),")
            else:
                compat_lines.append(f"                {rs_key}: Some({rust_string(v)}.into()),")
    ctk = compat.get("chatTemplateKwargs")
    if ctk:
        compat_lines.append(f'                chat_template_kwargs: Some(serde_json::from_str({rust_string(json.dumps(ctk))}).unwrap()),')
    cta = compat.get("chatTemplateArgs")
    if cta:
        compat_lines.append(f'                chat_template_args: Some(serde_json::from_str({rust_string(json.dumps(cta))}).unwrap()),')
    if compat_lines:
        lines.append("            compat: ModelCompat {")
        lines.extend(compat_lines)
        lines.append("                ..Default::default()")
        lines.append("            },")
    else:
        lines.append("            compat: ModelCompat::default(),")
    lines.append("        }")
    return "\n".join(lines)

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 scripts/generate_models.py /tmp/models.json", file=sys.stderr)
        sys.exit(1)
    
    input_path = Path(sys.argv[1])
    models = json.loads(input_path.read_text())
    
    all_models = []
    for provider in sorted(models.keys()):
        for model_id in sorted(models[provider].keys()):
            all_models.append(models[provider][model_id])
    
    total = len(all_models)
    providers = len(models)
    now = datetime.datetime.utcnow().isoformat()
    
    out = []
    out.append(f"//! Auto-generated model registry from @earendil-works/pi-ai. DO NOT EDIT.")
    out.append(f"//!")
    out.append(f"//! Source: models.generated.js ({total} models, {providers} providers)")
    out.append(f"//! Generated: {now}Z")
    out.append("")
    out.append("#![allow(clippy::approx_constant)]")
    out.append("")
    out.append("use std::collections::HashMap;")
    out.append("use crate::types::{Model, ModelCost, ModelCostTier, ModelCompat};")
    out.append("")
    chunk_size = 50
    for chunk_index, start in enumerate(range(0, total, chunk_size)):
        out.append(f"fn append_builtin_models_{chunk_index}(models: &mut Vec<Model>) {{")
        for m in all_models[start:start + chunk_size]:
            out.append("    models.push(")
            out.append(gen_model(m))
            out.append("    );")
        out.append("}")
        out.append("")

    out.append("/// Returns all built-in models from the upstream pi-ai registry.")
    out.append("pub fn builtin_models() -> Vec<Model> {")
    out.append(f"    let mut models = Vec::with_capacity({total});")
    for chunk_index, _ in enumerate(range(0, total, chunk_size)):
        out.append(f"    append_builtin_models_{chunk_index}(&mut models);")
    out.append("    models")
    out.append("}")
    
    output_path = Path(__file__).parent.parent / "src" / "models_generated.rs"
    output_path.write_text("\n".join(out) + "\n")
    print(f"Wrote {output_path} ({total} models, {providers} providers, {len(out)} lines)")

if __name__ == "__main__":
    main()
