use html5ever::tendril::TendrilSink;

fn html5_to_xhtml_void_elements(html: &str) -> String {
    const VOID: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];

    let mut result = html.to_string();
    for tag in VOID {
        // Replace <tag> with <tag/> and <tag ...attrs> with <tag ...attrs/>
        // We handle the two cases: self-contained (<br>) and with attributes (<img src="...">)
        let open = format!("<{}>", tag);
        let close_self = format!("<{}/>", tag);
        result = result.replace(&open, &close_self);

        // For tags with attributes: replace `<tag ...>` (not already ending in `/>`) with `<tag .../>`
        let prefix = format!("<{} ", tag);
        let mut out = String::with_capacity(result.len());
        let mut rest = result.as_str();
        while let Some(pos) = rest.find(&prefix) {
            out.push_str(&rest[..pos]);
            rest = &rest[pos..];
            if let Some(end) = rest.find('>') {
                let tag_str = &rest[..end + 1];
                if tag_str.ends_with("/>") {
                    out.push_str(tag_str);
                } else {
                    out.push_str(&rest[..end]);
                    out.push_str("/>");
                }
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
        out.push_str(rest);
        result = out;
    }
    result
}

fn html5_to_xhtml_void_elements_FIXED(html: &str) -> String {
    const VOID: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];

    // Regex explanation:
    // (?i) : case-insensitive
    // <(area|base|...) : match any of the void tags
    // ([^>]*?) : match any attributes (non-greedy)
    // /?> : match optional closing slash and then the closing bracket
    let pattern = format!(r"(?i)<({})(\b[^>]*?)?/?>", VOID.join("|"));
    let re = regex::Regex::new(&pattern).unwrap();
    
    re.replace_all(html, |caps: &regex::Captures| {
        let tag = caps.get(1).unwrap().as_str();
        let attrs = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        // We always want to return it as <tagattrs/>
        format!("<{}{}/>", tag, attrs)
    }).to_string()
}

fn main() {
    let raw_html = "<div><br\n><img src='foo.jpg'\t></div>";
    let dom = html5ever::parse_document(
        markup5ever_rcdom::RcDom::default(),
        Default::default(),
    ).one(raw_html);

    let mut bytes = Vec::new();
    let serializable: markup5ever_rcdom::SerializableHandle = dom.document.clone().into();
    html5ever::serialize(&mut bytes, &serializable, Default::default()).unwrap();
    let serialized = String::from_utf8(bytes).unwrap();
    
    println!("HTML5ever serialized output: {:?}", serialized);

    let test_cases = vec![
        ("<br>", "simple br"),
        ("<br >", "br with space"),
        ("<br\n>", "br with newline"),
        ("<br\t>", "br with tab"),
        ("<br/>", "already closed br"),
        ("<br />", "already closed br with space"),
        ("<br  >", "br with multiple spaces"),
        ("<img src=\"foo.jpg\">", "img without close"),
        ("<img src=\"foo.jpg\" >", "img with space before close"),
        ("<img src=\"foo.jpg\"/>", "img already closed"),
        ("<img\nsrc=\"foo.jpg\">", "img with newline after tag name"),
    ];

    println!("{:<30} | {:<20} | {:<20} | {:<10}", "Input", "Current Output", "Fixed Output", "Valid?");
    println!("{:-<30}-|-{:-<20}-|-{:-<20}-|-{:-<10}", "", "", "", "");

    for (input, _desc) in test_cases {
        let output = html5_to_xhtml_void_elements(input);
        let fixed_output = html5_to_xhtml_void_elements_FIXED(input);
        
        let is_valid = fixed_output.ends_with("/>");
        
        println!("{:<30} | {:<20} | {:<20} | {:<10}", 
            input.replace("\n", "\\n").replace("\t", "\\t"), 
            output.replace("\n", "\\n").replace("\t", "\\t"), 
            fixed_output.replace("\n", "\\n").replace("\t", "\\t"),
            if is_valid { "YES" } else { "NO" }
        );
    }
}
