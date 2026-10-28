object FormUpdateFirmware: TFormUpdateFirmware
  Left = 841
  Top = 539
  AutoSize = True
  BorderIcons = [biSystemMenu]
  BorderStyle = bsSingle
  BorderWidth = 30
  Caption = 'Update firmware'
  ClientHeight = 203
  ClientWidth = 300
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  OnCloseQuery = FormCloseQuery
  OnShow = FormShow
  PixelsPerInch = 96
  TextHeight = 13
  object ImageLeft: TImage
    Left = 0
    Top = 0
    Width = 124
    Height = 124
    AutoSize = True
  end
  object LabelMessage: TLabel
    Left = 136
    Top = 8
    Width = 164
    Height = 22
    Alignment = taCenter
    Caption = 'New firmwarre available'
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clWindowText
    Font.Height = -16
    Font.Name = 'Vrinda'
    Font.Style = []
    ParentFont = False
  end
  object LabelProgress: TLabel
    Left = 0
    Top = 157
    Width = 45
    Height = 13
    Caption = '0 Kb (0%)'
    Visible = False
  end
  object ButtonYes: TButton
    Left = 40
    Top = 168
    Width = 88
    Height = 35
    Caption = 'Yes'
    Default = True
    TabOrder = 0
    OnClick = ButtonYesClick
  end
  object ButtonCancel: TButton
    Left = 144
    Top = 168
    Width = 88
    Height = 35
    Cancel = True
    Caption = 'No'
    TabOrder = 1
    OnClick = ButtonCancelClick
  end
  object ProgressBar: TProgressBar
    Left = 0
    Top = 134
    Width = 241
    Height = 17
    TabOrder = 2
    Visible = False
  end
  object TimerCheckModel: TTimer
    OnTimer = TimerCheckModelTimer
    Left = 40
    Top = 64
  end
end
