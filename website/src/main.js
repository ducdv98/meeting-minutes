document.documentElement.classList.add('js');
const header = document.querySelector('.site-head');
const updateHeaderEdge = () => header?.classList.toggle('scrolled', window.scrollY > 0);
updateHeaderEdge();
window.addEventListener('scroll', updateHeaderEdge, { passive: true });

for (const [index, button] of [...document.querySelectorAll('.faq button')].entries()) {
  const answer = document.getElementById(button.getAttribute('aria-controls'));
  const expanded = index === 0;
  button.setAttribute('aria-expanded', String(expanded));
  answer.inert = !expanded;
  button.addEventListener('click', () => {
    const next = button.getAttribute('aria-expanded') !== 'true';
    button.setAttribute('aria-expanded', String(next));
    answer.inert = !next;
  });
}
for (const button of document.querySelectorAll('[data-copy]')) {
  button.addEventListener('click', async () => {
    const address = document.getElementById(button.dataset.copy);
    const status = button.closest('.copybox').nextElementSibling;
    try {
      await navigator.clipboard.writeText(address.textContent);
      status.textContent = 'Email address copied.';
    } catch {
      address.focus();
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(address);
      selection.removeAllRanges();
      selection.addRange(range);
      status.textContent = 'Email address selected. Copy it with Ctrl+C or your device’s copy command.';
    }
  });
}

if ('IntersectionObserver' in window && !window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
  const observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      entry.target.classList.add('reveal-visible');
      observer.unobserve(entry.target);
    }
  }, { threshold: 0.08 });
  for (const section of document.querySelectorAll('.section, .closing')) {
    if (section.getBoundingClientRect().top < window.innerHeight) continue;
    section.classList.add('reveal-pending');
    observer.observe(section);
  }

  const screenshotObserver = new IntersectionObserver(entries => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      entry.target.classList.add('fly-visible');
      screenshotObserver.unobserve(entry.target);
    }
  }, { threshold: 0.15 });
  for (const screenshot of document.querySelectorAll('.three > div, .split .screenshot, .support-shot .screenshot')) {
    if (screenshot.getBoundingClientRect().top < window.innerHeight) continue;
    screenshot.classList.add('fly-pending');
    screenshotObserver.observe(screenshot);
  }
}

const keyRequest = document.getElementById('key-request');
keyRequest?.addEventListener('submit', async event => {
  event.preventDefault();
  const email = keyRequest.elements.email.value.trim();
  const use = keyRequest.elements.use.value.trim();
  if (!email || !use) { keyRequest.reportValidity(); return; }
  const status = document.getElementById('key-request-status');
  const button = keyRequest.querySelector('button[type="submit"]');
  const openEmailDraft = () => {
    const subject = encodeURIComponent('Managed key request');
    const body = encodeURIComponent(`Email: ${email}\n\nHow I will use Managed transcription:\n${use}`);
    window.location.href = `mailto:admin@meeting-minutes.tech?subject=${subject}&body=${body}`;
  };
  const endpoint = keyRequest.dataset.endpoint;
  if (!endpoint) { openEmailDraft(); return; }
  button.disabled = true;
  status.textContent = 'Sending…';
  try {
    // text/plain keeps this a simple request; Apps Script sends no CORS headers, so the reply is opaque.
    await fetch(endpoint, { method: 'POST', mode: 'no-cors', headers: { 'Content-Type': 'text/plain;charset=utf-8' }, body: JSON.stringify({ email, use, website: keyRequest.elements.website.value }) });
    keyRequest.reset();
    status.textContent = 'Thanks. Your request is in. We review requests by hand and cannot promise a reply.';
  } catch {
    status.textContent = 'Sending failed. Opening an email draft instead.';
    openEmailDraft();
  } finally {
    button.disabled = false;
  }
});
