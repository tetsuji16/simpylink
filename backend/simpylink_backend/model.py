"""Model data structures + validation. Mirrors schemas/model.schema.json."""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


class ModelError(ValueError):
    pass


@dataclass
class Block:
    id: str
    type: str
    position: dict[str, float] = field(default_factory=lambda: {"x": 0.0, "y": 0.0})
    params: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, d: dict) -> "Block":
        if not isinstance(d, dict) or not d.get("id") or not d.get("type"):
            raise ModelError(f"block needs id and type: {d!r}")
        pos = d.get("position") or {"x": 0.0, "y": 0.0}
        return cls(id=str(d["id"]), type=str(d["type"]),
                   position={"x": float(pos.get("x", 0)), "y": float(pos.get("y", 0))},
                   params=dict(d.get("params") or {}))

    def to_dict(self) -> dict:
        return {"id": self.id, "type": self.type, "position": self.position, "params": self.params}


@dataclass(frozen=True)
class PortRef:
    block: str
    port: str

    @classmethod
    def from_dict(cls, d: dict) -> "PortRef":
        if not isinstance(d, dict) or "block" not in d or "port" not in d:
            raise ModelError(f"port ref needs block+port: {d!r}")
        return cls(block=str(d["block"]), port=str(d["port"]))

    def to_dict(self) -> dict:
        return {"block": self.block, "port": self.port}


@dataclass(frozen=True)
class Connection:
    source: PortRef
    target: PortRef

    @classmethod
    def from_dict(cls, d: dict) -> "Connection":
        return cls(source=PortRef.from_dict(d.get("from", {})), target=PortRef.from_dict(d.get("to", {})))

    def to_dict(self) -> dict:
        return {"from": self.source.to_dict(), "to": self.target.to_dict()}


@dataclass
class Model:
    name: str
    blocks: list[Block] = field(default_factory=list)
    connections: list[Connection] = field(default_factory=list)
    version: str = "0.1.0"

    @classmethod
    def from_dict(cls, d: dict) -> "Model":
        if not isinstance(d, dict):
            raise ModelError("model must be an object")
        blocks = [Block.from_dict(b) for b in d.get("blocks", [])]
        seen: set[str] = set()
        for b in blocks:
            if b.id in seen:
                raise ModelError(f"duplicate block id: {b.id}")
            seen.add(b.id)
        conns = [Connection.from_dict(c) for c in d.get("connections", [])]
        for c in conns:
            for ref in (c.source, c.target):
                if ref.block not in seen:
                    raise ModelError(f"connection references unknown block: {ref.block}")
        return cls(name=str(d.get("name", "untitled")), blocks=blocks,
                   connections=conns, version=str(d.get("version", "0.1.0")))

    def to_dict(self) -> dict:
        return {"name": self.name, "version": self.version,
                "blocks": [b.to_dict() for b in self.blocks],
                "connections": [c.to_dict() for c in self.connections]}

    def block_map(self) -> dict[str, Block]:
        return {b.id: b for b in self.blocks}
