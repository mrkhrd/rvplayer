object FormShowMessage: TFormShowMessage
  Left = 463
  Top = 399
  BorderIcons = []
  BorderStyle = bsDialog
  BorderWidth = 16
  ClientHeight = 100
  ClientWidth = 174
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  OnClose = FormClose
  OnKeyUp = FormKeyUp
  OnResize = FormResize
  PixelsPerInch = 96
  TextHeight = 13
  object LabelText: TLabel
    Left = 0
    Top = 0
    Width = 50
    Height = 13
    Caption = 'Label Text'
  end
  object ButtonCopy: TSpeedButton
    Left = 112
    Top = 64
    Width = 22
    Height = 22
    Cursor = crHandPoint
    AllowAllUp = True
    GroupIndex = 1
    Flat = True
    ParentShowHint = False
    ShowHint = True
    OnClick = ButtonCopyClick
  end
  object ButtonOk: TButton
    Left = 0
    Top = 64
    Width = 75
    Height = 30
    Cancel = True
    Caption = 'Ok'
    Default = True
    TabOrder = 0
    OnClick = ButtonOkClick
    OnKeyUp = FormKeyUp
  end
end
