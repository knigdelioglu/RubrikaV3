export function formatDate(isoString: string): string {
  return new Date(isoString).toLocaleDateString('tr-TR');
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
