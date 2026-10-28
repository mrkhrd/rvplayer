object FormRepair: TFormRepair
  Left = 409
  Top = 315
  Width = 435
  Height = 439
  HorzScrollBar.Visible = False
  VertScrollBar.Visible = False
  BorderWidth = 16
  Caption = 'Repair'
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  OnCreate = FormCreate
  OnResize = FormResize
  PixelsPerInch = 96
  TextHeight = 13
  object LabelBroken: TLabel
    Left = 0
    Top = 0
    Width = 34
    Height = 13
    Caption = 'Broken'
  end
  object LabelRef: TLabel
    Left = 0
    Top = 56
    Width = 17
    Height = 13
    Caption = 'Ref'
  end
  object LabelStatus: TLabel
    Left = 0
    Top = 336
    Width = 30
    Height = 13
    Caption = 'Status'
  end
  object EditBroken: TEdit
    Left = 0
    Top = 16
    Width = 353
    Height = 21
    TabOrder = 0
    OnChange = EditBrokenChange
    OnDblClick = ButtonFileBorkenClick
  end
  object EditRef: TEdit
    Left = 0
    Top = 72
    Width = 353
    Height = 21
    TabOrder = 2
    OnDblClick = ButtonFileBorkenClick
  end
  object ButtonRepair: TButton
    Left = 96
    Top = 296
    Width = 201
    Height = 33
    Caption = 'ButtonRepair'
    TabOrder = 5
    OnClick = ButtonRepairClick
  end
  object ProgressBar: TProgressBar
    Left = 0
    Top = 352
    Width = 387
    Height = 17
    Align = alBottom
    TabOrder = 6
  end
  object GroupBoxMethod: TGroupBox
    Left = 0
    Top = 120
    Width = 377
    Height = 161
    Caption = 'GroupBoxMethod'
    TabOrder = 4
    object LabelCaptionFast: TLabel
      Left = 32
      Top = 24
      Width = 185
      Height = 53
      AutoSize = False
      Caption = 'CaptionFast'
      WordWrap = True
      OnClick = LabelCaptionFastClick
    end
    object LabelCaptionFull: TLabel
      Left = 32
      Top = 76
      Width = 201
      Height = 53
      AutoSize = False
      Caption = 'CaptionFull'
      WordWrap = True
      OnClick = LabelCaptionFullClick
    end
    object LabelResult: TLabel
      Left = 24
      Top = 132
      Width = 30
      Height = 13
      Caption = 'Result'
      OnClick = LabelCaptionFullClick
    end
    object RadioButtonFast: TRadioButton
      Left = 10
      Top = 24
      Width = 18
      Height = 18
      Checked = True
      TabOrder = 0
      TabStop = True
      OnClick = LabelCaptionFastClick
    end
    object RadioButtonFull: TRadioButton
      Left = 10
      Top = 76
      Width = 18
      Height = 18
      TabOrder = 1
      OnClick = LabelCaptionFullClick
    end
    object EditResult: TEdit
      Left = 64
      Top = 128
      Width = 265
      Height = 21
      TabOrder = 2
      OnDblClick = ButtonFileResultClick
    end
    object ButtonFileResult: TButton
      Left = 336
      Top = 128
      Width = 21
      Height = 21
      Caption = '...'
      TabOrder = 3
      OnClick = ButtonFileResultClick
    end
  end
  object ButtonFileBorken: TButton
    Left = 360
    Top = 16
    Width = 21
    Height = 21
    Caption = '...'
    TabOrder = 1
    OnClick = ButtonFileBorkenClick
  end
  object ButtonFileRef: TButton
    Left = 360
    Top = 72
    Width = 21
    Height = 21
    Caption = '...'
    TabOrder = 3
    OnClick = ButtonFileBorkenClick
  end
  object TimerConsole: TTimer
    Interval = 50
    OnTimer = TimerConsoleTimer
    Top = 304
  end
end
