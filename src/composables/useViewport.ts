const IOS = CSS.supports("-webkit-touch-callout", "none");

function lock() {
  const meta = document.querySelector<HTMLMetaElement>('meta[name="viewport"]');
  if (meta && !meta.content.includes("maximum-scale")) meta.content += ", maximum-scale=1";
}

if (IOS) {
  lock();
  new MutationObserver(lock).observe(document.head, { childList: true, subtree: true, attributes: true, attributeFilter: ["content"] });
}

export {};
