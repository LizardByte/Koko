/** Generates small original-content books used by the reader's real-browser checks. */
import { mkdir, writeFile } from 'node:fs/promises';
import { deflateRawSync } from 'node:zlib';

const directory = new URL('./fixtures/reader/', import.meta.url);
await mkdir(directory, { recursive: true });
const crc32 = (buffer) => {
  let crc = 0xffffffff;
  for (const byte of buffer) { crc ^= byte; for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0); }
  return (crc ^ 0xffffffff) >>> 0;
};
function zip(files) {
  const chunks = [], index = [];
  let offset = 0;
  for (const [name, text] of Object.entries(files)) {
    const filename = Buffer.from(name), raw = Buffer.from(text), method = name === 'mimetype' ? 0 : 8, compressed = method ? deflateRawSync(raw) : raw;
    const header = Buffer.alloc(30); header.writeUInt32LE(0x04034b50); header.writeUInt16LE(20, 4); header.writeUInt16LE(method, 8);
    header.writeUInt32LE(crc32(raw), 14); header.writeUInt32LE(compressed.length, 18); header.writeUInt32LE(raw.length, 22); header.writeUInt16LE(filename.length, 26);
    const central = Buffer.alloc(46); central.writeUInt32LE(0x02014b50); central.writeUInt16LE(20, 4); central.writeUInt16LE(20, 6); central.writeUInt16LE(method, 10);
    central.writeUInt32LE(crc32(raw), 16); central.writeUInt32LE(compressed.length, 20); central.writeUInt32LE(raw.length, 24); central.writeUInt16LE(filename.length, 28); central.writeUInt32LE(offset, 42);
    chunks.push(header, filename, compressed); index.push(central, filename); offset += header.length + filename.length + compressed.length;
  }
  const central = Buffer.concat(index), end = Buffer.alloc(22); end.writeUInt32LE(0x06054b50);
  end.writeUInt16LE(index.length / 2, 8); end.writeUInt16LE(index.length / 2, 10); end.writeUInt32LE(central.length, 12); end.writeUInt32LE(offset, 16);
  return Buffer.concat([...chunks, central, end]);
}
const paragraphText = Array.from({ length: 45 }, (_, i) => `Paragraph ${i + 1}. This original book tests comfortable reading, text selection, font changes, search, and saved positions. A bookmark should return to this location after closing the reader.`);
const paragraphs = paragraphText.map((text) => `<p>${text}</p>`).join('');
const chapter = (name, extra = '') => `<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>${name}</title></head><body><h1>${name}</h1>${extra}${paragraphs}</body></html>`;
await writeFile(new URL('sample.epub', directory), zip({
  mimetype: 'application/epub+zip',
  'META-INF/container.xml': '<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>',
  'book.opf': '<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">koko-reader-fixture</dc:identifier><dc:title>A Place to Read</dc:title><dc:language>en</dc:language><dc:creator>Koko</dc:creator></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="two.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="one"/><itemref idref="two"/></spine></package>',
  'nav.xhtml': '<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head><body><nav epub:type="toc"><ol><li><a href="one.xhtml">A quiet beginning</a></li><li><a href="two.xhtml">The next chapter</a></li></ol></nav></body></html>',
  'one.xhtml': chapter('A quiet beginning', '<script>parent.__bookScriptExecuted=true</script><img src="data:image/png;base64," onerror="parent.__bookEventExecuted=true"/>'),
  'two.xhtml': chapter('The next chapter', '<p>The lantern is a unique search phrase.</p>'),
}));
const comic = (number, color) => `<svg xmlns="http://www.w3.org/2000/svg" width="900" height="1200" viewBox="0 0 900 1200"><rect width="900" height="1200" fill="${color}"/><rect x="50" y="60" width="800" height="1080" rx="20" fill="#faf4e8"/><text x="100" y="160" font-family="sans-serif" font-size="48">Koko comic test</text><text x="100" y="280" font-family="sans-serif" font-size="72">Page ${number}</text><path d="M100 900L400 430L780 900Z" fill="${color}"/><text x="100" y="1060" font-family="sans-serif" font-size="28">Natural order · zoom · bookmarks</text></svg>`;
await writeFile(new URL('sample.cbz', directory), zip({ 'page10.svg': comic(10, '#497a91'), 'page2.svg': comic(2, '#5c9564'), 'page1.svg': comic(1, '#b56f50'), '__MACOSX/._page0.svg': 'ignored' }));
await writeFile(new URL('sample.fb2', directory), `<?xml version="1.0"?><FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0"><description><title-info><genre>fiction</genre><author><first-name>Koko</first-name><last-name>Test</last-name></author><book-title>FictionBook test</book-title><lang>en</lang></title-info></description><body><section><title><p>First section</p></title>${paragraphs}</section><section><title><p>Second section</p></title><p>The lantern is a unique search phrase.</p>${paragraphs}</section></body></FictionBook>\n`);
await writeFile(new URL('sample.txt', directory), `A plain text book\n\n${paragraphText.join('\n\n')}\n`);
const stream = (text) => `BT /F1 26 Tf 50 700 Td (${text}) Tj ET`;
const first = stream('Koko PDF reader - Page 1'), second = stream('The lantern - Page 2');
const objects = ['<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>',
  '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 7 0 R >> >> /Contents 4 0 R >>', `<< /Length ${first.length} >>\nstream\n${first}\nendstream`,
  '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>', `<< /Length ${second.length} >>\nstream\n${second}\nendstream`, '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'];
let pdf = '%PDF-1.4\n', offsets = [0];
objects.forEach((object, i) => { offsets.push(Buffer.byteLength(pdf)); pdf += `${i + 1} 0 obj\n${object}\nendobj\n`; });
// CRLF keeps each PDF xref record at its required 20 bytes without trailing spaces.
const xref = Buffer.byteLength(pdf), entries = offsets.slice(1).map((offset) => `${String(offset).padStart(10, '0')} 00000 n\r\n`).join('');
pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f\r\n${entries}trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
await writeFile(new URL('sample.pdf', directory), pdf);
