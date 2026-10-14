import type { ReactNode } from "react";

type Block = { kind: "paragraph" | "heading" | "plain"; text: string } | { kind: "bullets" | "numbers"; items: string[]; start?: number };

function inline(text: string): ReactNode[] {
  if (/<\/?[A-Za-z][^>\n]*>/.test(text)) return [text];
  const parts: ReactNode[] = [];
  let plain = "";
  const flush = () => { if (plain) { parts.push(plain); plain = ""; } };
  for (let i = 0; i < text.length;) {
    const rest = text.slice(i);
    const unsupported = rest.match(/^!?\[[^\]\n]*\]\([^\)\n]*\)/) ?? rest.match(/^<\/?[A-Za-z][^>\n]*>/);
    if (unsupported) { plain += unsupported[0]; i += unsupported[0].length; continue; }
    if (text[i] === "\\" && i + 1 < text.length) { plain += text[i + 1]; i += 2; continue; }
    if (text[i] === "\n") { flush(); parts.push(<br key={i} />); i++; continue; }
    if (text[i] === "`") {
      const end = text.indexOf("`", i + 1);
      if (end > i + 1) { flush(); parts.push(<code key={i} className="rounded-sm bg-panel px-1 text-sm">{text.slice(i + 1, end)}</code>); i = end + 1; continue; }
    }
    const marker = text.startsWith("**", i) ? "**" : text.startsWith("__", i) ? "__" : text[i] === "*" || text[i] === "_" ? text[i] : null;
    if (marker) {
      const end = text.indexOf(marker, i + marker.length);
      if (end > i + marker.length) {
        flush();
        const content = inline(text.slice(i + marker.length, end));
        parts.push(marker.length === 2 ? <strong key={i}>{content}</strong> : <em key={i}>{content}</em>);
        i = end + marker.length; continue;
      }
    }
    plain += text[i]; i++;
  }
  flush();
  return parts;
}

function blocks(text: string): Block[] {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const result: Block[] = [];
  const list = (line: string) => line.match(/^\s*(?:([-*+])\s+|(\d+)\.\s+)(.*)$/);
  const plain = (line: string) => line.includes("|") || /<\/?[A-Za-z][^>\n]*>/.test(line);
  for (let i = 0; i < lines.length;) {
    const line = lines[i];
    if (!line.trim()) { i++; continue; }
    if (/^\s*(```|~~~)/.test(line)) {
      const fence = line.trim().slice(0, 3);
      const content = [line]; i++;
      while (i < lines.length) { content.push(lines[i]); if (lines[i++].trim().startsWith(fence)) break; }
      result.push({ kind: "plain", text: content.join("\n") }); continue;
    }
    if (plain(line)) {
      const content = [line]; i++;
      while (i < lines.length && plain(lines[i])) content.push(lines[i++]);
      result.push({ kind: "plain", text: content.join("\n") }); continue;
    }
    const heading = line.match(/^\s*#{1,6}\s+(.+)$/);
    if (heading) { result.push({ kind: "heading", text: heading[1] }); i++; continue; }
    const first = list(line);
    if (first) {
      const kind = first[2] ? "numbers" : "bullets";
      const items: string[] = [];
      const start = first[2] ? Number(first[2]) : undefined;
      while (i < lines.length) {
        const match = list(lines[i]);
        if (!match || (match[2] ? "numbers" : "bullets") !== kind) break;
        const item = match[3];
        if (kind === "numbers") items.push(...item.split(/\s+(?=\d+\.\s+)/).map((part, index) => index ? part.replace(/^\d+\.\s+/, "") : part));
        else items.push(item);
        i++;
      }
      result.push({ kind, items, start }); continue;
    }
    const content = [line]; i++;
    while (i < lines.length && lines[i].trim() && !list(lines[i]) && !/^\s*#{1,6}\s+/.test(lines[i]) && !/^\s*(```|~~~)/.test(lines[i]) && !plain(lines[i])) content.push(lines[i++]);
    result.push({ kind: "paragraph", text: content.join("\n") });
  }
  return result;
}

export default function Markdown({ text, trailing }: { text: string; trailing?: ReactNode }) {
  const content = blocks(text);
  const suffix = (last: boolean) => last && trailing && (!Array.isArray(trailing) || trailing.length) ? <>{" "}{trailing}</> : null;
  return <div className="grid gap-2">{content.map((block, index) => {
    const last = index === content.length - 1;
    if ("items" in block) {
      const Tag = block.kind === "numbers" ? "ol" : "ul";
      return <Tag key={index} start={block.start} className={`m-0 grid gap-1 pl-[22px] ${block.kind === "numbers" ? "list-decimal" : "list-disc"}`}>{block.items.map((item, itemIndex) => <li key={itemIndex}>{inline(item)}{suffix(last && itemIndex === block.items.length - 1)}</li>)}</Tag>;
    }
    const plainLines = block.kind === "plain" ? block.text.split("\n") : [];
    return <p key={index} className="m-0">{block.kind === "heading" ? <strong>{inline(block.text)}</strong> : block.kind === "plain" ? plainLines.map((line, lineIndex) => <span key={lineIndex}>{line}{lineIndex < plainLines.length - 1 && <br />}</span>) : inline(block.text)}{suffix(last)}</p>;
  })}</div>;
}
