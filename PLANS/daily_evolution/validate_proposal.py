#!/usr/bin/env python3
"""
Daily Evolution Proposal Validator

Validates proposal JSON against the JSON schema.
Can also validate the markdown proposal by extracting embedded JSON metadata.

Usage:
    python validate_proposal.py --proposal PLANS/DAILY_EVOLUTION_PROPOSAL_20261028.md
    python validate_proposal.py --json proposal.json
"""

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any, Dict, Optional

try:
    import jsonschema
except ImportError:
    print("Error: jsonschema not installed. Run: pip install jsonschema")
    sys.exit(1)


def load_schema(schema_path: Path) -> Dict[str, Any]:
    """Load JSON schema from file."""
    with open(schema_path, "r", encoding="utf-8") as f:
        return json.load(f)


def extract_json_from_markdown(markdown_path: Path) -> Optional[Dict[str, Any]]:
    """Extract JSON metadata from markdown proposal file."""
    content = markdown_path.read_text(encoding="utf-8")

    # Find the JSON block at the end of the file
    json_match = re.search(r"```json\n(.*?)\n```", content, re.DOTALL)
    if not json_match:
        return None

    try:
        return json.loads(json_match.group(1))
    except json.JSONDecodeError as e:
        print(f"Error parsing JSON from markdown: {e}")
        return None


def validate_proposal(proposal: Dict[str, Any], schema: Dict[str, Any]) -> tuple[bool, list]:
    """Validate proposal against schema. Returns (is_valid, errors)."""
    validator = jsonschema.Draft7Validator(schema)
    errors = list(validator.iter_errors(proposal))

    if not errors:
        return True, []

    error_messages = []
    for error in errors:
        path = " -> ".join(str(p) for p in error.absolute_path)
        error_messages.append(f"{path}: {error.message}")

    return False, error_messages


def main():
    parser = argparse.ArgumentParser(description="Validate Daily Evolution Proposal")
    parser.add_argument("--proposal", help="Path to markdown proposal file")
    parser.add_argument("--json", help="Path to JSON proposal file")
    parser.add_argument(
        "--schema",
        default="PLANS/daily_evolution_schema.json",
        help="Path to JSON schema file",
    )
    args = parser.parse_args()

    if not args.proposal and not args.json:
        parser.error("Either --proposal or --json is required")

    # Load schema
    schema_path = Path(args.schema)
    if not schema_path.exists():
        print(f"Error: Schema not found at {schema_path}")
        sys.exit(1)

    schema = load_schema(schema_path)

    # Load proposal
    if args.proposal:
        proposal_path = Path(args.proposal)
        if not proposal_path.exists():
            print(f"Error: Proposal not found at {proposal_path}")
            sys.exit(1)

        proposal = extract_json_from_markdown(proposal_path)
        if proposal is None:
            print("Error: Could not extract JSON metadata from markdown")
            sys.exit(1)
        print(f"Validating markdown proposal: {proposal_path}")
    else:
        json_path = Path(args.json)
        if not json_path.exists():
            print(f"Error: JSON file not found at {json_path}")
            sys.exit(1)

        with open(json_path, "r", encoding="utf-8") as f:
            proposal = json.load(f)
        print(f"Validating JSON proposal: {json_path}")

    # Validate
    is_valid, errors = validate_proposal(proposal, schema)

    if is_valid:
        print("✅ Proposal is VALID")
        print(f"   Template version: {proposal.get('template_version')}")
        print(f"   Channel: {proposal.get('channel_id')}")
        print(f"   Date: {proposal.get('date')}")
        print(f"   Action items: {proposal['metadata']['action_items_count']}")
        print(f"   Blockers: {proposal['metadata']['blockers_count']}")
        print(f"   Decisions: {proposal['metadata']['decisions_count']}")
        sys.exit(0)
    else:
        print("❌ Proposal is INVALID")
        for error in errors:
            print(f"  - {error}")
        sys.exit(1)


if __name__ == "__main__":
    main()
