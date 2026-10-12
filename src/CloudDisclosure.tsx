import { PROVIDER_NAMES, type Disclosure } from "./ai";

export default function CloudDisclosure({ disclosure, onSend, onCancel }: { disclosure: Disclosure; onSend(): void; onCancel(): void }) {
  const label = PROVIDER_NAMES[disclosure.provider];
  return <section role="dialog" aria-label={`Send to ${label}?`} className="cloud-disclosure">
    <h2>Send to {label}?</h2>
    <p>Provider: {label}</p><p>Model: {disclosure.model}</p><p>Endpoint: {disclosure.endpoint}</p>
    <p>{disclosure.characters} characters</p>
    <div className="cloud-disclosure-text"><h3>System text</h3><pre>{disclosure.system}</pre>
      {disclosure.messages.map((message, index) => <div key={index}><h3>{message.role}</h3><pre>{message.content}</pre></div>)}</div>
    <p>Your consent is remembered for this Meeting; each send still needs your click.</p>
    <button type="button" onClick={onSend}>Send</button><button type="button" onClick={onCancel}>Cancel</button>
  </section>;
}
