//! Małe PDF-y budowane w testach (bez binarnych plików wzorcowych): obiekty, strumienie, xref.

/// Czcionka Helvetica z polskimi znakami przez `Differences` (128 ą, 129 ę, 130 ł, 131 ś, 132 ż,
/// 133 ź, 134 ć, 135 ń, 136 Ą przez `uni0104`, 137 nieznana nazwa); ó z WinAnsi (0xF3).
pub(super) const POLISH_FONT: &str = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding \
     << /Type /Encoding /BaseEncoding /WinAnsiEncoding /Differences [128 /aogonek /eogonek /lslash \
     /sacute /zdotaccent /zacute /cacute /nacute /uni0104 /g77] >> >>";

/// Mapa ToUnicode dla czcionki Type0 (Identity-H).
pub(super) const TO_UNICODE: &[u8] = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap
/CMapName /Adobe-Identity-UCS def
1 begincodespacerange <0000> <FFFF> endcodespacerange
7 beginbfchar
<0001> <0105> <0002> <0142> <0003> <017C> <0004> <0141> <0005> <00F3> <0006> <0064> <0007> <017A>
endbfchar
1 beginbfrange <0010> <0029> <0041> endbfrange
endcmap CMapName currentdict /CMap defineresource pop end end";

/// Dokument PDF w budowie.
pub(super) struct Pdf {
    objects: Vec<Vec<u8>>,
}

impl Pdf {
    pub(super) fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    /// Rezerwuje numer obiektu (treść — później przez `set`).
    pub(super) fn reserve(&mut self) -> usize {
        self.objects.push(b"null".to_vec());
        self.objects.len()
    }

    pub(super) fn set(&mut self, id: usize, body: impl AsRef<[u8]>) {
        self.objects[id - 1] = body.as_ref().to_vec();
    }

    pub(super) fn add(&mut self, body: impl AsRef<[u8]>) -> usize {
        let id = self.reserve();
        self.set(id, body);
        id
    }

    pub(super) fn stream(&mut self, dict: &str, data: &[u8]) -> usize {
        let mut body = format!("<< /Length {} {dict} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.add(body)
    }

    /// Strony z treścią `contents` i wspólnymi zasobami; obiekty stron powstają od końca, więc ich
    /// numery są odwrotne do kolejności w drzewie stron. Zwraca katalog.
    pub(super) fn pages(&mut self, contents: &[&[u8]], resources: &str) -> usize {
        let parent = self.reserve();
        let mut ids = vec![0; contents.len()];
        for (i, content) in contents.iter().enumerate().rev() {
            let c = self.stream("", content);
            ids[i] = self.add(format!(
                "<< /Type /Page /Parent {parent} 0 R /MediaBox [0 0 595 842] /Contents {c} 0 R >>"
            ));
        }
        let kids: Vec<String> = ids.iter().map(|id| format!("{id} 0 R")).collect();
        self.set(
            parent,
            format!(
                "<< /Type /Pages /Kids [{}] /Count {} /Resources {resources} >>",
                kids.join(" "),
                contents.len()
            ),
        );
        self.add(format!("<< /Type /Catalog /Pages {parent} 0 R >>"))
    }

    /// Plik z tabelą xref i trailerem (z identyfikatorem pliku — potrzebnym do szyfrowania).
    pub(super) fn finish(&self, root: usize) -> Vec<u8> {
        let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        let size = self.objects.len() + 1;
        out.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
        for offset in offsets {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        let id = "<0123456789ABCDEF0123456789ABCDEF>";
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {size} /Root {root} 0 R /ID [{id} {id}] >>\nstartxref\n{xref}\n%%EOF\n"
            )
            .as_bytes(),
        );
        out
    }
}

/// Dokument z czcionką `/F1` (polska, prosta) i `/F2` (Type0 z ToUnicode) oraz stronami.
pub(super) fn with_fonts(contents: &[&[u8]]) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f1 = pdf.add(POLISH_FONT);
    let cmap = pdf.stream("", TO_UNICODE);
    let cid = pdf.add(
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Arial /CIDSystemInfo << /Registry (Adobe) \
         /Ordering (Identity) /Supplement 0 >> /DW 500 >>",
    );
    let f2 = pdf.add(format!(
        "<< /Type /Font /Subtype /Type0 /BaseFont /Arial /Encoding /Identity-H /DescendantFonts \
         [{cid} 0 R] /ToUnicode {cmap} 0 R >>"
    ));
    let root = pdf.pages(
        contents,
        &format!("<< /Font << /F1 {f1} 0 R /F2 {f2} 0 R >> >>"),
    );
    pdf.finish(root)
}
