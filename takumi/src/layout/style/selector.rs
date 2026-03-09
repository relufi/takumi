use cssparser::*;
use precomputed_hash::PrecomputedHash;
use selectors::parser::{
  Component, NonTSPseudoClass, ParseRelative, PseudoElement, Selector, SelectorImpl, SelectorList,
  SelectorParseErrorKind,
};
use std::{
  borrow::Cow,
  fmt::{self, Write},
  rc::Rc,
};
use taffy::Size;

use crate::keyframes::{KeyframePreludeParseError, parse_keyframe_prelude};
use crate::{
  layout::{
    Viewport,
    style::{CalcArena, FromCss, KeyframeRule, KeyframesRule, Length, StyleDeclarationBlock},
  },
  rendering::Sizing,
};

#[derive(Debug, Clone)]
pub enum CssSelectorParseError<'i> {
  #[allow(dead_code)]
  Basic(BasicParseErrorKind<'i>),
  #[allow(dead_code)]
  Property(Cow<'i, str>),
  #[allow(dead_code)]
  Selector(SelectorParseErrorKind<'i>),
  #[allow(dead_code)]
  UnsupportedSelectorFeature(&'static str),
}

impl<'i> From<SelectorParseErrorKind<'i>> for CssSelectorParseError<'i> {
  fn from(err: SelectorParseErrorKind<'i>) -> Self {
    CssSelectorParseError::Selector(err)
  }
}

impl<'i> From<Cow<'i, str>> for CssSelectorParseError<'i> {
  fn from(err: Cow<'i, str>) -> Self {
    CssSelectorParseError::Property(err)
  }
}

impl<'i> From<KeyframePreludeParseError<'i>> for CssSelectorParseError<'i> {
  fn from(_err: KeyframePreludeParseError<'i>) -> Self {
    Self::Basic(BasicParseErrorKind::QualifiedRuleInvalid)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TakumiIdent(pub String);

impl From<&str> for TakumiIdent {
  fn from(s: &str) -> Self {
    Self(s.to_owned())
  }
}

impl AsRef<str> for TakumiIdent {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl ToCss for TakumiIdent {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    serialize_identifier(&self.0, dest)
  }
}

impl PrecomputedHash for TakumiIdent {
  fn precomputed_hash(&self) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for byte in self.0.as_bytes() {
      hash ^= u32::from(byte.to_ascii_lowercase());
      hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
  }
}

#[derive(Debug, Clone)]
pub struct TakumiSelectorImpl;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DummyPseudoClass {
  #[default]
  Hover,
}

impl ToCss for DummyPseudoClass {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    match self {
      DummyPseudoClass::Hover => dest.write_str(":hover"),
    }
  }
}

impl NonTSPseudoClass for DummyPseudoClass {
  type Impl = TakumiSelectorImpl;
  fn is_active_or_hover(&self) -> bool {
    *self == DummyPseudoClass::Hover
  }
  fn is_user_action_state(&self) -> bool {
    true
  }
}

// TODO: support pseudo elements
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DummyPseudoElement {
  #[default]
  Noop,
}

impl ToCss for DummyPseudoElement {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    match self {
      DummyPseudoElement::Noop => dest.write_str("::noop"),
    }
  }
}

impl PseudoElement for DummyPseudoElement {
  type Impl = TakumiSelectorImpl;
}

impl SelectorImpl for TakumiSelectorImpl {
  type ExtraMatchingData<'a> = ();
  type AttrValue = TakumiIdent;
  type Identifier = TakumiIdent;
  type LocalName = TakumiIdent;
  type NamespaceUrl = TakumiIdent;
  type NamespacePrefix = TakumiIdent;
  type BorrowedNamespaceUrl = TakumiIdent;
  type BorrowedLocalName = TakumiIdent;
  type NonTSPseudoClass = DummyPseudoClass;
  type PseudoElement = DummyPseudoElement;
}

struct TakumiSelectorParser;

impl<'i> selectors::Parser<'i> for TakumiSelectorParser {
  type Impl = TakumiSelectorImpl;
  type Error = CssSelectorParseError<'i>;
}

fn selector_contains_unsupported_features(selector: &Selector<TakumiSelectorImpl>) -> bool {
  selector
    .iter_raw_match_order()
    .any(|component| match component {
      Component::AttributeInNoNamespaceExists { .. }
      | Component::AttributeInNoNamespace { .. }
      | Component::AttributeOther(_) => true,
      Component::Negation(list) | Component::Is(list) | Component::Where(list) => list
        .slice()
        .iter()
        .any(selector_contains_unsupported_features),
      Component::Has(relatives) => relatives
        .iter()
        .any(|rel| selector_contains_unsupported_features(&rel.selector)),
      Component::Slotted(inner) => selector_contains_unsupported_features(inner),
      Component::Host(Some(inner)) => selector_contains_unsupported_features(inner),
      _ => false,
    })
}

fn ensure_supported_selector_list<'i>(
  selectors: &SelectorList<TakumiSelectorImpl>,
) -> Result<(), CssSelectorParseError<'i>> {
  if selectors
    .slice()
    .iter()
    .any(selector_contains_unsupported_features)
  {
    return Err(CssSelectorParseError::UnsupportedSelectorFeature(
      "attribute selectors are not supported",
    ));
  }

  Ok(())
}

pub struct StyleDeclarationParser;

impl<'i> DeclarationParser<'i> for StyleDeclarationParser {
  type Declaration = StyleDeclarationBlock;
  type Error = CssSelectorParseError<'i>;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
    _state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
    let mut declarations = StyleDeclarationBlock::parse(&name, input).map_err(ParseError::into)?;
    let important = input.try_parse(parse_important).is_ok();
    if important {
      for declaration in &declarations.declarations {
        declarations.importance.insert_declaration(declaration);
      }
    }
    Ok(declarations)
  }
}

impl<'i> QualifiedRuleParser<'i> for StyleDeclarationParser {
  type Prelude = ();
  type QualifiedRule = StyleDeclarationBlock;
  type Error = CssSelectorParseError<'i>;
}

impl<'i> AtRuleParser<'i> for StyleDeclarationParser {
  type Prelude = ();
  type AtRule = StyleDeclarationBlock;
  type Error = CssSelectorParseError<'i>;
}

impl<'i> RuleBodyItemParser<'i, StyleDeclarationBlock, CssSelectorParseError<'i>>
  for StyleDeclarationParser
{
  fn parse_qualified(&self) -> bool {
    false
  }
  fn parse_declarations(&self) -> bool {
    true
  }
}

struct KeyframeDeclarationParser;

impl<'i> DeclarationParser<'i> for KeyframeDeclarationParser {
  type Declaration = StyleDeclarationBlock;
  type Error = CssSelectorParseError<'i>;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
    _state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
    let declarations = StyleDeclarationBlock::parse(&name, input).map_err(ParseError::into)?;
    let _ = input.try_parse(parse_important);
    Ok(declarations)
  }
}

impl<'i> QualifiedRuleParser<'i> for KeyframeDeclarationParser {
  type Prelude = ();
  type QualifiedRule = StyleDeclarationBlock;
  type Error = CssSelectorParseError<'i>;
}

impl<'i> AtRuleParser<'i> for KeyframeDeclarationParser {
  type Prelude = ();
  type AtRule = StyleDeclarationBlock;
  type Error = CssSelectorParseError<'i>;
}

impl<'i> RuleBodyItemParser<'i, StyleDeclarationBlock, CssSelectorParseError<'i>>
  for KeyframeDeclarationParser
{
  fn parse_qualified(&self) -> bool {
    false
  }

  fn parse_declarations(&self) -> bool {
    true
  }
}

struct KeyframeRuleParser;

impl<'i> QualifiedRuleParser<'i> for KeyframeRuleParser {
  type Prelude = Vec<f32>;
  type QualifiedRule = KeyframeRule;
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    parse_keyframe_prelude(input)
  }

  fn parse_block<'t>(
    &mut self,
    offsets: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    let mut declaration_parser = KeyframeDeclarationParser;
    let parser = RuleBodyParser::new(input, &mut declaration_parser);
    let mut declarations = StyleDeclarationBlock::default();
    for block in parser.filter_map(Result::ok) {
      declarations.append(block);
    }

    Ok(KeyframeRule {
      offsets,
      declarations,
    })
  }
}

impl<'i> AtRuleParser<'i> for KeyframeRuleParser {
  type Prelude = ();
  type AtRule = KeyframeRule;
  type Error = CssSelectorParseError<'i>;
}

struct TakumiRuleParser;

#[derive(Debug, Clone, PartialEq)]
enum MediaType {
  All,
  Screen,
  Unsupported(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaFeatureComparison {
  Equal,
  Min,
  Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaOrientation {
  Portrait,
  Landscape,
}

#[derive(Debug, Clone, PartialEq)]
enum MediaFeature {
  Width(MediaFeatureComparison, Length<false>),
  Height(MediaFeatureComparison, Length<false>),
  Orientation(MediaOrientation),
}

#[derive(Debug, Clone, PartialEq)]
struct MediaQuery {
  media_type: MediaType,
  features: Vec<MediaFeature>,
  negated: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct MediaQueryList {
  queries: Vec<MediaQuery>,
}

impl MediaFeature {
  fn matches(&self, viewport: Viewport, sizing: &Sizing) -> bool {
    match self {
      Self::Width(comparison, value) => viewport.width.is_some_and(|width| {
        compare_media_feature(*comparison, width as f32, value.to_px(sizing, width as f32))
      }),
      Self::Height(comparison, value) => viewport.height.is_some_and(|height| {
        compare_media_feature(
          *comparison,
          height as f32,
          value.to_px(sizing, height as f32),
        )
      }),
      Self::Orientation(MediaOrientation::Portrait) => viewport
        .width
        .zip(viewport.height)
        .is_some_and(|(width, height)| height >= width),
      Self::Orientation(MediaOrientation::Landscape) => viewport
        .width
        .zip(viewport.height)
        .is_some_and(|(width, height)| width > height),
    }
  }
}

impl MediaQuery {
  fn matches(&self, viewport: Viewport, sizing: &Sizing) -> bool {
    let media_type_matches = match &self.media_type {
      MediaType::All | MediaType::Screen => true,
      MediaType::Unsupported(_) => false,
    };

    let mut is_match = media_type_matches
      && self
        .features
        .iter()
        .all(|feature| feature.matches(viewport, sizing));

    if self.negated {
      is_match = !is_match;
    }

    is_match
  }
}

impl MediaQueryList {
  pub(crate) fn matches(&self, viewport: Viewport) -> bool {
    if self.queries.is_empty() {
      return true;
    }

    let sizing = Sizing {
      viewport,
      container_size: Size::NONE,
      font_size: viewport.font_size,
      calc_arena: Rc::new(CalcArena::default()),
    };

    self
      .queries
      .iter()
      .any(|query| query.matches(viewport, &sizing))
  }
}

fn compare_media_feature(comparison: MediaFeatureComparison, actual: f32, expected: f32) -> bool {
  match comparison {
    MediaFeatureComparison::Equal => (actual - expected).abs() <= f32::EPSILON,
    MediaFeatureComparison::Min => actual >= expected,
    MediaFeatureComparison::Max => actual <= expected,
  }
}

fn parse_media_query_list<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<MediaQueryList, ParseError<'i, CssSelectorParseError<'i>>> {
  let mut queries = Vec::new();

  loop {
    queries.push(parse_media_query(input)?);

    if input.try_parse(Parser::expect_comma).is_err() {
      break;
    }
  }

  Ok(MediaQueryList { queries })
}

fn parse_media_query<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<MediaQuery, ParseError<'i, CssSelectorParseError<'i>>> {
  let mut negated = false;
  let mut media_type = MediaType::All;
  let mut features = Vec::new();
  let mut has_explicit_media_type = false;

  if let Ok(keyword) = input.try_parse(Parser::expect_ident_cloned) {
    if keyword.eq_ignore_ascii_case("not") {
      negated = true;
      media_type = parse_media_type(input.expect_ident_cloned()?);
      has_explicit_media_type = true;
    } else if keyword.eq_ignore_ascii_case("only") {
      media_type = parse_media_type(input.expect_ident_cloned()?);
      has_explicit_media_type = true;
    } else {
      media_type = parse_media_type(keyword);
      has_explicit_media_type = true;
    }
  }

  if input
    .try_parse(|input| parse_media_feature_block(input, &mut features))
    .is_ok()
  {
    while input
      .try_parse(|input| input.expect_ident_matching("and"))
      .is_ok()
    {
      parse_media_feature_block(input, &mut features)?;
    }
  } else if has_explicit_media_type {
    while input
      .try_parse(|input| input.expect_ident_matching("and"))
      .is_ok()
    {
      parse_media_feature_block(input, &mut features)?;
    }
  }

  Ok(MediaQuery {
    media_type,
    features,
    negated,
  })
}

fn parse_media_type(name: CowRcStr<'_>) -> MediaType {
  if name.eq_ignore_ascii_case("all") {
    MediaType::All
  } else if name.eq_ignore_ascii_case("screen") {
    MediaType::Screen
  } else {
    MediaType::Unsupported(name.to_string())
  }
}

fn parse_media_feature_block<'i, 't>(
  input: &mut Parser<'i, 't>,
  features: &mut Vec<MediaFeature>,
) -> Result<(), ParseError<'i, CssSelectorParseError<'i>>> {
  let location = input.current_source_location();
  let token = input.next()?;
  match token {
    Token::ParenthesisBlock => input.parse_nested_block(|input| {
      features.push(parse_media_feature(input)?);
      Ok(())
    }),
    _ => Err(location.new_unexpected_token_error(token.clone())),
  }
}

fn parse_media_feature<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<MediaFeature, ParseError<'i, CssSelectorParseError<'i>>> {
  let feature_name = input.expect_ident_cloned()?;
  input.expect_colon()?;

  if feature_name.eq_ignore_ascii_case("orientation") {
    let orientation = input.expect_ident_cloned()?;
    return if orientation.eq_ignore_ascii_case("portrait") {
      Ok(MediaFeature::Orientation(MediaOrientation::Portrait))
    } else if orientation.eq_ignore_ascii_case("landscape") {
      Ok(MediaFeature::Orientation(MediaOrientation::Landscape))
    } else {
      Err(
        input.new_error(BasicParseErrorKind::UnexpectedToken(Token::Ident(
          orientation,
        ))),
      )
    };
  }

  let comparison = if feature_name.eq_ignore_ascii_case("min-width")
    || feature_name.eq_ignore_ascii_case("min-height")
  {
    MediaFeatureComparison::Min
  } else if feature_name.eq_ignore_ascii_case("max-width")
    || feature_name.eq_ignore_ascii_case("max-height")
  {
    MediaFeatureComparison::Max
  } else {
    MediaFeatureComparison::Equal
  };

  let length = Length::<false>::from_css(input).map_err(ParseError::into)?;

  if feature_name.eq_ignore_ascii_case("width")
    || feature_name.eq_ignore_ascii_case("min-width")
    || feature_name.eq_ignore_ascii_case("max-width")
  {
    Ok(MediaFeature::Width(comparison, length))
  } else if feature_name.eq_ignore_ascii_case("height")
    || feature_name.eq_ignore_ascii_case("min-height")
    || feature_name.eq_ignore_ascii_case("max-height")
  {
    Ok(MediaFeature::Height(comparison, length))
  } else {
    Err(
      input.new_custom_error(CssSelectorParseError::UnsupportedSelectorFeature(
        "unsupported media feature",
      )),
    )
  }
}

#[derive(Debug, Clone)]
enum AtRulePrelude {
  Keyframes(String),
  Media(MediaQueryList),
}

#[derive(Debug, Clone)]
pub struct CssRule {
  pub selectors: SelectorList<TakumiSelectorImpl>,
  pub normal_declarations: StyleDeclarationBlock,
  pub important_declarations: StyleDeclarationBlock,
  pub media_queries: Option<MediaQueryList>,
}

#[derive(Debug, Clone)]
pub enum StyleSheetRule {
  Style(Box<CssRule>),
  Media(Vec<CssRule>),
  Keyframes(KeyframesRule),
}

impl<'i> QualifiedRuleParser<'i> for TakumiRuleParser {
  type Prelude = SelectorList<TakumiSelectorImpl>;
  type QualifiedRule = StyleSheetRule;
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    let selectors = SelectorList::parse(&TakumiSelectorParser, input, ParseRelative::No)?;
    ensure_supported_selector_list(&selectors).map_err(|err| input.new_custom_error(err))?;
    Ok(selectors)
  }

  fn parse_block<'t>(
    &mut self,
    selectors: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    let mut normal_declarations = StyleDeclarationBlock::default();
    let mut important_declarations = StyleDeclarationBlock::default();
    let mut decl_parser = StyleDeclarationParser;
    let parser = RuleBodyParser::new(input, &mut decl_parser);
    for res in parser {
      match res {
        Ok(declarations) => {
          if declarations.importance.is_empty() {
            normal_declarations.append(declarations);
          } else {
            important_declarations.append(declarations);
          }
        }
        Err((_error, _declaration)) => continue,
      }
    }
    Ok(StyleSheetRule::Style(Box::new(CssRule {
      selectors,
      normal_declarations,
      important_declarations,
      media_queries: None,
    })))
  }
}

impl<'i> AtRuleParser<'i> for TakumiRuleParser {
  type Prelude = AtRulePrelude;
  type AtRule = StyleSheetRule;
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    if name.eq_ignore_ascii_case("keyframes") {
      return Ok(AtRulePrelude::Keyframes(
        input.expect_ident_or_string()?.to_string(),
      ));
    }

    if name.eq_ignore_ascii_case("media") {
      return parse_media_query_list(input).map(AtRulePrelude::Media);
    }

    Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name)))
  }

  fn parse_block<'t>(
    &mut self,
    prelude: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
    match prelude {
      AtRulePrelude::Keyframes(name) => {
        let mut parser = KeyframeRuleParser;
        let rule_list_parser = StyleSheetParser::new(input, &mut parser);
        let keyframes = rule_list_parser.filter_map(Result::ok).collect::<Vec<_>>();

        Ok(StyleSheetRule::Keyframes(KeyframesRule { name, keyframes }))
      }
      AtRulePrelude::Media(media_queries) => {
        let mut parser = TakumiRuleParser;
        let rule_list_parser = StyleSheetParser::new(input, &mut parser);
        let mut rules = Vec::new();

        for rule in rule_list_parser.filter_map(Result::ok) {
          if let StyleSheetRule::Style(mut rule) = rule {
            rule.media_queries = Some(media_queries.clone());
            rules.push(*rule);
          }
        }

        Ok(StyleSheetRule::Media(rules))
      }
    }
  }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StyleSheet {
  pub rules: Vec<CssRule>,
  pub keyframes: Vec<KeyframesRule>,
}

impl StyleSheet {
  pub(crate) fn parse_list<'a, I>(stylesheets: I) -> impl Iterator<Item = Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    stylesheets.into_iter().map(Self::parse)
  }

  pub(crate) fn parse(css: &str) -> Self {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut rule_parser = TakumiRuleParser;
    let mut rules = Vec::new();
    let mut keyframes = Vec::new();

    let rule_list_parser = StyleSheetParser::new(&mut parser, &mut rule_parser);

    for rule in rule_list_parser {
      match rule {
        Ok(StyleSheetRule::Style(rule)) => rules.push(*rule),
        Ok(StyleSheetRule::Media(media_rules)) => rules.extend(media_rules),
        Ok(StyleSheetRule::Keyframes(rule)) => keyframes.push(rule),
        Err((_error, _slice)) => continue,
      }
    }

    Self { rules, keyframes }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::{
    Color, ColorInput, ComputedStyle, Length, Style, StyleDeclaration, StyleDeclarationBlock,
  };

  fn computed_style_from_declarations(declarations: &StyleDeclarationBlock) -> ComputedStyle {
    let mut style = Style::default();
    for declaration in &declarations.declarations {
      declaration.merge_into_ref(&mut style);
    }
    style.inherit(&ComputedStyle::default())
  }

  #[test]
  fn test_parse_stylesheet() {
    let css = r#"
            .box {
                width: 100px;
                color: red;
            }
        "#;
    let sheet = StyleSheet::parse(css);
    assert_eq!(sheet.rules.len(), 1);
    let rule = &sheet.rules[0];

    assert_eq!(rule.selectors.slice().len(), 1);
    assert_eq!(
      computed_style_from_declarations(&rule.normal_declarations).width,
      Length::Px(100.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_compound_selectors_specificity() {
    let sheet = StyleSheet::parse(
      r#"
        div.box { width: 10px; }
        #hero .label { height: 20px; }
      "#,
    );
    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(sheet.rules[0].selectors.slice().len(), 1);
    assert_eq!(sheet.rules[1].selectors.slice().len(), 1);
    assert!(sheet.rules[0].selectors.slice()[0].specificity() > 0);
    assert!(
      sheet.rules[1].selectors.slice()[0].specificity()
        > sheet.rules[0].selectors.slice()[0].specificity()
    );
  }

  #[test]
  fn test_parse_stylesheet_multiple_rules() {
    let sheet = StyleSheet::parse(
      r#"
        .a { width: 10px; }
        .b { height: 20px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(10.0)
    );
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[1].normal_declarations).height,
      Length::Px(20.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_multiple_selectors_in_rule() {
    let sheet = StyleSheet::parse(
      r#"
        .a, .b { width: 12px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].selectors.slice().len(), 2);
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(12.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_important_declaration() {
    let sheet = StyleSheet::parse(
      r#"
        .a { width: 10px !important; height: 20px; }
      "#,
    );

    let rule = &sheet.rules[0];
    assert_eq!(
      computed_style_from_declarations(&rule.important_declarations).width,
      Length::Px(10.0)
    );
    assert_eq!(
      computed_style_from_declarations(&rule.normal_declarations).height,
      Length::Px(20.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_shorthand_clears_prior_longhand() {
    let sheet = StyleSheet::parse(
      r#"
        .a { padding-left: 4px; padding: 10px; }
      "#,
    );

    let declarations = &sheet.rules[0].normal_declarations;
    assert_eq!(declarations.declarations.len(), 5);
    assert_eq!(
      declarations.declarations[0],
      StyleDeclaration::padding_left(Length::Px(4.0))
    );
    assert_eq!(
      declarations.declarations[1],
      StyleDeclaration::padding_top(Length::Px(10.0))
    );
    assert_eq!(
      declarations.declarations[2],
      StyleDeclaration::padding_right(Length::Px(10.0))
    );
    assert_eq!(
      declarations.declarations[3],
      StyleDeclaration::padding_bottom(Length::Px(10.0))
    );
    assert_eq!(
      declarations.declarations[4],
      StyleDeclaration::padding_left(Length::Px(10.0))
    );
  }

  #[test]
  fn test_parse_stylesheet_webkit_alias_property() {
    let sheet = StyleSheet::parse(
      r#"
        .a { -webkit-text-fill-color: rgb(255, 0, 0); }
      "#,
    );

    let style = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(
      style.webkit_text_fill_color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_stylesheet_unknown_property_does_not_drop_supported_declarations() {
    let sheet = StyleSheet::parse(
      r#"
        .a { --local-token: 1; width: 14px; unsupported-prop: 2; height: 6px; }
      "#,
    );

    let style = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(style.width, Length::Px(14.0));
    assert_eq!(style.height, Length::Px(6.0));
  }

  #[test]
  fn test_unsupported_attribute_selector_rule_is_rejected() {
    let sheet = StyleSheet::parse(
      r#"
        [data-kind="hero"] { width: 10px; }
      "#,
    );

    assert!(sheet.rules.is_empty());
  }

  #[test]
  fn test_unsupported_pseudo_selector_rule_is_rejected() {
    let sheet = StyleSheet::parse(
      r#"
        .a:hover { width: 10px; }
      "#,
    );

    assert!(sheet.rules.is_empty());
  }

  #[test]
  fn test_parse_keyframes_rule() {
    let sheet = StyleSheet::parse(
      r#"
        @keyframes fade {
          from { opacity: 0; }
          50% { opacity: 0.5; }
          to { opacity: 1; }
        }
      "#,
    );

    assert!(sheet.rules.is_empty());
    assert_eq!(sheet.keyframes.len(), 1);
    assert_eq!(sheet.keyframes[0].name, "fade");
    assert_eq!(sheet.keyframes[0].keyframes.len(), 3);
    assert_eq!(sheet.keyframes[0].keyframes[0].offsets, vec![0.0]);
    assert_eq!(sheet.keyframes[0].keyframes[1].offsets, vec![0.5]);
    assert_eq!(sheet.keyframes[0].keyframes[2].offsets, vec![1.0]);
  }

  #[test]
  fn test_parse_media_rule_with_viewport_features() {
    let sheet = StyleSheet::parse(
      r#"
        @media screen and (min-width: 600px) and (orientation: landscape) {
          .card { width: 100px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert!(sheet.keyframes.is_empty());
    assert!(
      sheet.rules[0]
        .media_queries
        .as_ref()
        .is_some_and(|media| media.matches(Viewport::new(Some(800), Some(600))))
    );
    assert!(
      !sheet.rules[0]
        .media_queries
        .as_ref()
        .is_some_and(|media| media.matches(Viewport::new(Some(500), Some(800))))
    );
  }

  #[test]
  fn test_parse_media_rule_with_comma_list() {
    let sheet = StyleSheet::parse(
      r#"
        @media (max-width: 480px), (min-width: 1024px) {
          .card { width: 100px; }
        }
      "#,
    );

    let Some(media) = sheet.rules[0].media_queries.as_ref() else {
      unreachable!("expected media queries on parsed rule");
    };
    assert!(media.matches(Viewport::new(Some(400), Some(800))));
    assert!(media.matches(Viewport::new(Some(1280), Some(800))));
    assert!(!media.matches(Viewport::new(Some(800), Some(800))));
  }
}
