use cssparser::{BasicParseErrorKind, Parser, Token};
use parley::FontFeature;
use std::borrow::Cow;

use crate::layout::style::{CssToken, FromCss, MakeComputed, ParseResult};

/// Controls OpenType font features via CSS font-feature-settings property.
///
/// This allows enabling/disabling specific typographic features in OpenType fonts
/// such as ligatures, kerning, small caps, and other advanced typography features.
pub type FontFeatureSettings = Box<[FontFeature]>;

impl MakeComputed for FontFeatureSettings {}

impl<'i> FromCss<'i> for FontFeatureSettings {
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
        return Err(
          input.new_error::<Cow<str>>(BasicParseErrorKind::UnexpectedToken(Token::QuotedString(
            err_token,
          ))),
        );
      }
      let tag = swash::tag_from_str_lossy(tag_name);
      let start_location = input.current_source_location();
      let value = if input.is_exhausted() {
        1f32
      } else {
        match input.next()? {
          Token::Ident(st) if st.as_ref() == "on" => 1f32,
          Token::Ident(st) if st.as_ref() == "off" => 0f32,
          Token::Number { value, .. } => *value,
          token => {
            return Err(start_location.new_unexpected_token_error::<Cow<str>>(token.clone()));
          }
        }
      };
      if value > u16::MAX as f32 || value < 0f32 {
        return Err(input.new_custom_error("Invalid font feature value"));
      };
      Ok(FontFeature {
        tag,
        value: value as u16,
      })
    })?;
    Ok(list.into_boxed_slice())
  }

  fn from_str(source: &'i str) -> ParseResult<'i, Self> {
    Ok(Box::from_iter(FontFeature::parse_list(source)))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Keyword("normal"), CssToken::Token("string")]
  }
}
