object FormAbout: TFormAbout
  Left = 496
  Top = 307
  AutoSize = True
  BorderIcons = [biSystemMenu]
  BorderStyle = bsSingle
  BorderWidth = 10
  Caption = 'About'
  ClientHeight = 322
  ClientWidth = 300
  Color = clWhite
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  OnCreate = FormCreate
  OnDestroy = FormDestroy
  OnShow = FormShow
  PixelsPerInch = 96
  TextHeight = 13
  object LabelTitle: TLabel
    Left = 16
    Top = 168
    Width = 204
    Height = 40
    Caption = 'Program Name'
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clWindowText
    Font.Height = -29
    Font.Name = 'Comic Sans MS'
    Font.Style = [fsBold]
    ParentFont = False
  end
  object LabelVer: TLabel
    Left = 128
    Top = 207
    Width = 26
    Height = 13
    Caption = 'v1.0'
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clWindowText
    Font.Height = -11
    Font.Name = 'MS Sans Serif'
    Font.Style = [fsBold]
    ParentFont = False
  end
  object LabelName: TLabel
    Left = 24
    Top = 246
    Width = 62
    Height = 13
    Alignment = taCenter
    Caption = 'Author Name'
    Transparent = True
  end
  object LabelMail: TLabel
    Left = 24
    Top = 265
    Width = 83
    Height = 13
    Cursor = crHandPoint
    Hint = 'Send mail'
    Alignment = taCenter
    Caption = 'user@server.com'
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clBlack
    Font.Height = -11
    Font.Name = 'MS Sans Serif'
    Font.Style = []
    ParentFont = False
    ParentShowHint = False
    ShowHint = True
    Transparent = True
    OnClick = LabelMailClick
    OnMouseEnter = LabelMailMouseEnter
    OnMouseLeave = LabelMailMouseLeave
  end
  object LabelCity: TLabel
    Left = 24
    Top = 284
    Width = 68
    Height = 13
    Alignment = taCenter
    Caption = 'Author Region'
  end
  object ImageLogo: TImage
    Left = 0
    Top = 0
    Width = 300
    Height = 158
    AutoSize = True
  end
  object LabelSite: TLabel
    Left = 41
    Top = 306
    Width = 102
    Height = 16
    Cursor = crHandPoint
    Hint = 'Visit Site'
    Alignment = taCenter
    Caption = 'http://site.com'
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clBlack
    Font.Height = -13
    Font.Name = 'Verdana'
    Font.Style = []
    ParentFont = False
    ParentShowHint = False
    ShowHint = True
    Transparent = True
    OnClick = LabelSiteClick
    OnMouseEnter = LabelMailMouseEnter
    OnMouseLeave = LabelMailMouseLeave
  end
end
