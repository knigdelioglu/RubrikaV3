function parseSafeDate(value: string | null | undefined): Date | null {
  if (!value?.trim()) return null;

  // Rust's RFC3339 formatter may emit more than JavaScript's portable
  // three-digit millisecond precision. Truncating extra fractional digits
  // keeps the timestamp valid in older WKWebView implementations too.
  const normalized = value
    .trim()
    .replace(/\.(\d{3})\d+(?=(?:Z|[+-]\d{2}:\d{2})$)/, '.$1');
  const date = new Date(normalized);
  return Number.isNaN(date.getTime()) ? null : date;
}

export function formatDate(isoString: string): string {
  const date = parseSafeDate(isoString);
  return date ? date.toLocaleDateString('tr-TR') : 'Tarih bilinmiyor';
}

export function formatDateTime(value: string | null | undefined): string | null {
  const date = parseSafeDate(value);
  if (!date) return null;

  try {
    return new Intl.DateTimeFormat('tr-TR', {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(date);
  } catch {
    // Keep rendering safe on older WebViews that do not support dateStyle or
    // timeStyle even when the date itself is valid.
    try {
      return date.toLocaleString('tr-TR', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
      });
    } catch {
      return null;
    }
  }
}

export function formatPageCount(pageCount?: number | null): string {
  if (!pageCount || pageCount <= 0) {
    return 'Henüz bilinmiyor';
  }

  return pageCount.toString();
}

export function formatPageRange(pageNumbers: number[]): string {
  if (pageNumbers.length === 0) {
    return '-';
  }

  const sorted = [...pageNumbers].sort((a, b) => a - b);
  const [firstPage, ...remainingPages] = sorted;
  if (firstPage === undefined) {
    return '-';
  }

  const ranges: string[] = [];
  let start = firstPage;
  let end = firstPage;

  for (const page of remainingPages) {
    if (page === end + 1) {
      end = page;
      continue;
    }

    ranges.push(start === end ? `${start}` : `${start}-${end}`);
    start = page;
    end = page;
  }

  ranges.push(start === end ? `${start}` : `${start}-${end}`);
  return ranges.join(', ');
}
