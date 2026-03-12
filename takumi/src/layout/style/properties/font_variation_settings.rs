use crate::layout::style::{CssToken, FromCss, MakeComputed, ParseResult};
use cssparser::{BasicParseErrorKind, Parser};
use parley::FontVariation;

/// Controls variable font axis values via CSS font-variation-settings property.
///
/// This allows fine-grained control over variable font characteristics like weight,
/// width, slant, and other custom axes defined in the font.
pub type FontVariationSettings = Box<[FontVariation]>;

impl MakeComputed for FontVariationSettings {}

impl<'i> FromCss<'i> for FontVariationSettings {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(Box::new([]));
    }
    let list = input.parse_comma_separated(|input| {
      let tag_name = input.expect_string()?;
      if tag_name.len() != 4 || !tag_name.is_ascii() {
        let err_token = tag_name.clone();
        return Err(input.new_error::<std::borrow::Cow<str>>(
          BasicParseErrorKind::UnexpectedToken(cssparser::Token::QuotedString(err_token)),
        ));
      }
      let tag = swash::tag_from_str_lossy(tag_name);
      let value = input.expect_number()?;
      Ok(FontVariation { tag, value })
    })?;
    Ok(list.into_boxed_slice())
  }

  fn from_str(source: &'i str) -> ParseResult<'i, Self> {
    Ok(Box::from_iter(FontVariation::parse_list(source)))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Keyword("normal"), CssToken::Token("string")]
  }
}
