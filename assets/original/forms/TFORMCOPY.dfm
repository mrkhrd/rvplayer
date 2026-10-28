object FormCopy: TFormCopy
  Left = 193
  Top = 125
  Width = 461
  Height = 207
  HorzScrollBar.Visible = False
  VertScrollBar.Visible = False
  BorderIcons = [biSystemMenu, biMinimize]
  Caption = 'FormCopy'
  Color = clBtnFace
  Constraints.MinWidth = 350
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  Position = poDefaultPosOnly
  OnClose = FormClose
  OnCreate = FormCreate
  OnResize = FormResize
  PixelsPerInch = 96
  TextHeight = 13
  object LabelFile: TLabel
    Left = 56
    Top = 6
    Width = 16
    Height = 13
    Caption = 'File'
  end
  object LabelSize: TLabel
    Left = 56
    Top = 24
    Width = 20
    Height = 13
    Caption = 'Size'
  end
  object LabelSpeed: TLabel
    Left = 232
    Top = 24
    Width = 31
    Height = 13
    Caption = 'Speed'
  end
  object LabelRemain: TLabel
    Left = 232
    Top = 42
    Width = 41
    Height = 13
    Caption = 'Remains'
  end
  object LabelFileCaption: TLabel
    Left = 24
    Top = 6
    Width = 23
    Height = 13
    Caption = 'FileC'
  end
  object LabelSizeCaption: TLabel
    Left = 16
    Top = 24
    Width = 27
    Height = 13
    Caption = 'SizeC'
  end
  object LabelSpeedCaption: TLabel
    Left = 184
    Top = 24
    Width = 38
    Height = 13
    Caption = 'SpeedC'
  end
  object LabelRemainCaption: TLabel
    Left = 176
    Top = 42
    Width = 48
    Height = 13
    Caption = 'RemainsC'
  end
  object LabelTime: TLabel
    Left = 56
    Top = 42
    Width = 23
    Height = 13
    Caption = 'Time'
  end
  object LabelTimeCaption: TLabel
    Left = 16
    Top = 42
    Width = 30
    Height = 13
    Caption = 'TimeC'
  end
  object ButtonOpenFolder: TSpeedButton
    Left = 184
    Top = 1
    Width = 89
    Height = 21
    Caption = 'Open Folder'
    Flat = True
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clNavy
    Font.Height = -11
    Font.Name = 'MS Sans Serif'
    Font.Style = [fsUnderline]
    ParentFont = False
    OnClick = ButtonOpenFolderClick
  end
  object ButtonPause: TSpeedButton
    Left = 112
    Top = 39
    Width = 41
    Height = 19
    AllowAllUp = True
    GroupIndex = 1
    Caption = 'Pause'
    Flat = True
    Font.Charset = DEFAULT_CHARSET
    Font.Color = clNavy
    Font.Height = -11
    Font.Name = 'MS Sans Serif'
    Font.Style = [fsUnderline]
    ParentFont = False
    OnClick = ButtonPauseClick
  end
  object ProgressBar: TProgressBar
    Left = 8
    Top = 63
    Width = 273
    Height = 17
    TabOrder = 0
  end
  object Timer: TTimer
    Enabled = False
    Interval = 100
    OnTimer = TimerTimer
    Left = 104
  end
end
