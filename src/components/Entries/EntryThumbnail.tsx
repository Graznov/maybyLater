import { useMemo } from 'react';

/** Images are embedded in the editor HTML, including data URLs from pasted files. */
export function EntryThumbnail({ content }: { content: string }) {
  const images = useMemo(() => {
    const doc = new DOMParser().parseFromString(content, 'text/html');
    return Array.from(doc.querySelectorAll('img[src]'))
      .map((image) => image.getAttribute('src') || '')
      .filter((src) => /^(data:image\/(?:png|jpeg|gif|webp|bmp);base64,|https?:\/\/)/i.test(src));
  }, [content]);
  if (!images.length) return null;
  return <span className="entry-thumbnail" aria-label={`${images.length} image(s)`}>
    <img src={images[0]} alt="" loading="lazy" decoding="async" />
    {images.length > 1 && <span className="entry-thumbnail__count">+{images.length - 1}</span>}
  </span>;
}
