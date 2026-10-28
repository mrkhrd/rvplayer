object FormUpdateExe: TFormUpdateExe
  Left = 850
  Top = 162
  AutoSize = True
  BorderIcons = [biSystemMenu]
  BorderStyle = bsSingle
  BorderWidth = 20
  Caption = 'Update'
  ClientHeight = 161
  ClientWidth = 443
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  OnShow = FormShow
  PixelsPerInch = 96
  TextHeight = 13
  object Label1: TLabel
    Left = 128
    Top = 24
    Width = 185
    Height = 16
    Alignment = taCenter
    AutoSize = False
    Caption = 'New version ready'
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clWindowText
    Font.Height = -13
    Font.Name = 'MS Sans Serif'
    Font.Style = []
    ParentFont = False
  end
  object Label2: TLabel
    Left = 128
    Top = 64
    Width = 185
    Height = 41
    Alignment = taCenter
    AutoSize = False
    Caption = 'Do you want to update?'
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clWindowText
    Font.Height = -13
    Font.Name = 'MS Sans Serif'
    Font.Style = []
    ParentFont = False
    WordWrap = True
  end
  object LabelVersion: TLabel
    Left = 128
    Top = 42
    Width = 185
    Height = 16
    Alignment = taCenter
    AutoSize = False
    Caption = '1.0'
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clWindowText
    Font.Height = -13
    Font.Name = 'MS Sans Serif'
    Font.Style = []
    ParentFont = False
  end
  object LabelProgress: TLabel
    Left = 172
    Top = 124
    Width = 45
    Height = 13
    Caption = '0 Kb (0%)'
    Visible = False
  end
  object ImageLeft: TImage
    Left = 0
    Top = 0
    Width = 124
    Height = 124
    AutoSize = True
  end
  object ImageRight: TImage
    Left = 319
    Top = 0
    Width = 124
    Height = 124
    AutoSize = True
  end
  object ProgressBar: TProgressBar
    Left = 128
    Top = 105
    Width = 121
    Height = 17
    TabOrder = 0
    Visible = False
  end
  object ButtonAuto: TButton
    Left = 0
    Top = 136
    Width = 169
    Height = 25
    Caption = 'Automatically'
    Default = True
    TabOrder = 1
    OnClick = ButtonAutoClick
  end
  object ButtonBrowser: TButton
    Left = 184
    Top = 136
    Width = 169
    Height = 25
    Caption = 'Download by browser'
    TabOrder = 2
    OnClick = ButtonBrowserClick
  end
  object ButtonCancel: TButton
    Left = 368
    Top = 136
    Width = 75
    Height = 25
    Cancel = True
    Caption = 'Cancel'
    TabOrder = 3
    OnClick = ButtonCancelClick
  end
end
