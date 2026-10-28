object FormSetupSubtitleTemplate: TFormSetupSubtitleTemplate
  Left = 436
  Top = 481
  Width = 450
  Height = 280
  BorderIcons = [biSystemMenu]
  BorderStyle = bsSizeToolWin
  Caption = 'Setup subtitle template'
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  OnResize = FormResize
  OnShow = FormShow
  PixelsPerInch = 96
  TextHeight = 13
  object LabelPreview: TLabel
    Left = 0
    Top = 108
    Width = 434
    Height = 91
    Align = alBottom
    Alignment = taCenter
    AutoSize = False
    Caption = 'Example'
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clWindowText
    Font.Height = -16
    Font.Name = 'Arial'
    Font.Style = [fsBold]
    ParentFont = False
  end
  object MemoTemplate: TMemo
    Left = 0
    Top = 17
    Width = 434
    Height = 91
    Align = alClient
    Alignment = taCenter
    Lines.Strings = (
      'MemoTemplate'
    )
    ScrollBars = ssVertical
    TabOrder = 0
    WordWrap = False
    OnKeyUp = MemoTemplateKeyUp
  end
  object PanelBottom: TPanel
    Left = 0
    Top = 199
    Width = 434
    Height = 43
    Align = alBottom
    BevelOuter = bvNone
    TabOrder = 1
    object LabelLegeng: TLabel
      Left = 80
      Top = 0
      Width = 250
      Height = 39
      Caption = '%y%  %m%  %d%  %h%  %mi%  %s%  %date%  %time%
%lat%  %lng%  %alt%
%speed%  %bearing%  %track%'
    end
    object ButtonOk: TButton
      Left = 7
      Top = 7
      Width = 65
      Height = 30
      Caption = 'Ok'
      Default = True
      TabOrder = 0
      OnClick = ButtonOkClick
      OnKeyUp = MemoTemplateKeyUp
    end
  end
  object PanelTop: TPanel
    Left = 0
    Top = 0
    Width = 434
    Height = 17
    Align = alTop
    BevelOuter = bvNone
    TabOrder = 2
    object CheckBoxDefault: TCheckBox
      Left = 2
      Top = 0
      Width = 359
      Height = 17
      Caption = 'Default'
      TabOrder = 0
      OnClick = CheckBoxDefaultClick
      OnKeyUp = MemoTemplateKeyUp
    end
  end
end
